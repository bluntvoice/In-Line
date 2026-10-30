// Fixed In-Line installation metadata, used only inside the finite file transaction.
// Shell links are changed under exclusive handles; foreign links are never replaced.
using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;
using System.Security.AccessControl;
using System.Security.Cryptography;
using System.Text;
using Microsoft.Win32;

namespace InLine.Installer {
 public sealed class MetadataOptions {
  public string Version, Publisher, BundleId, Desktop, Programs, StartMenuFolder, Hive;
  public bool StartMenu, DesktopShortcut, RemoveShortcuts;
  public int EstimatedSize;
 }
 public sealed class RegistryValueState {
  public string Name; public RegistryValueKind Kind; public string Text; public string[] Strings; public byte[] Bytes; public long Number;
 }
 public sealed class RegistryState {
  public string Path; public bool Existed; public string Acl;
  public List<RegistryValueState> Values=new List<RegistryValueState>();
 }
 public sealed class RegistryChange { public RegistryState Before, After; }
 public sealed class ShortcutChange {
  public string Path, BeforeHash, AfterHash, Acl; public byte[] Before, After;
  public long Created, Accessed, Written;
 }
 public sealed class MetadataState {
  public string Root, Hive, Desktop, Programs, StartMenuFolder;
  public List<RegistryChange> Registry=new List<RegistryChange>();
  public List<ShortcutChange> Shortcuts=new List<ShortcutChange>();
 }
 public sealed class MetadataTransaction : IDisposable {
  const string Arp="Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\In Line";
  const string Vendor="Software\\github\\In Line";
  readonly List<FileStream> streams=new List<FileStream>();
  readonly List<bool> created=new List<bool>();
  readonly List<string> directories=new List<string>();
  public MetadataState State;
  bool completed;
  static void Need(bool value,string message) { Guard.Require(value,"Installation metadata: "+message); }
  static bool SamePath(string a,string b) { return String.Equals(a,b,StringComparison.OrdinalIgnoreCase); }
  static string Digest(byte[] bytes) { using(var h=SHA256.Create()) return BitConverter.ToString(h.ComputeHash(bytes)).Replace("-",""); }
  static byte[] Read(FileStream s) { s.Position=0; using(var m=new MemoryStream()) { s.CopyTo(m); return m.ToArray(); } }
  static void Write(FileStream s,byte[] bytes) { s.Position=0; s.Write(bytes,0,bytes.Length); s.SetLength(bytes.Length); s.Flush(true); }
  static bool ValuesEqual(RegistryValueState a,RegistryValueState b) {
   if(a==null||b==null) return a==null&&b==null;
   if(a.Kind!=b.Kind||a.Text!=b.Text||a.Number!=b.Number) return false;
   if((a.Bytes==null)!=(b.Bytes==null)||(a.Strings==null)!=(b.Strings==null)) return false;
   if(a.Bytes!=null&&Convert.ToBase64String(a.Bytes)!=Convert.ToBase64String(b.Bytes)) return false;
   if(a.Strings!=null&&(a.Strings.Length!=b.Strings.Length||String.Join("\0",a.Strings)!=String.Join("\0",b.Strings))) return false;
   return true;
  }
  static RegistryValueState Find(RegistryState state,string name) { return state.Values.Find(delegate(RegistryValueState v) { return SamePath(v.Name,name); }); }
  static RegistryKey Base(string hive) {
   Need(hive=="CurrentUser"||hive=="LocalMachine","unsupported registry scope.");
   return RegistryKey.OpenBaseKey(hive=="CurrentUser"?RegistryHive.CurrentUser:RegistryHive.LocalMachine,RegistryView.Registry64);
  }
  static RegistryState Capture(string hive,string path) {
   var state=new RegistryState { Path=path };
   using(var b=Base(hive)) using(var k=b.OpenSubKey(path,false)) {
    if(k==null) return state;
    Need(k.SubKeyCount==0,"unexpected product registry subkeys; preserved for inspection.");
    state.Existed=true; state.Acl=k.GetAccessControl(AccessControlSections.Access).GetSecurityDescriptorSddlForm(AccessControlSections.Access);
    foreach(string name in k.GetValueNames()) {
     var v=new RegistryValueState { Name=name,Kind=k.GetValueKind(name) }; var value=k.GetValue(name,null,RegistryValueOptions.DoNotExpandEnvironmentNames);
     if(value is string) v.Text=(string)value;
     else if(value is int) v.Number=(int)value;
     else if(value is long) v.Number=(long)value;
     else if(value is byte[]) v.Bytes=(byte[])value;
     else if(value is string[]) v.Strings=(string[])value;
     else Need(false,"unrecognized product registry value type.");
     state.Values.Add(v);
    }
   }
   return state;
  }
  static bool RegistryEqual(RegistryState a,RegistryState b) {
   if(a.Existed!=b.Existed||a.Values.Count!=b.Values.Count) return false;
   foreach(var v in a.Values) if(!ValuesEqual(v,Find(b,v.Name))) return false;
   return true;
  }
  static object Value(RegistryValueState v) {
   if(v.Kind==RegistryValueKind.String||v.Kind==RegistryValueKind.ExpandString) return v.Text;
   if(v.Kind==RegistryValueKind.DWord) return (int)v.Number;
   if(v.Kind==RegistryValueKind.QWord) return v.Number;
   if(v.Kind==RegistryValueKind.MultiString) return v.Strings;
   return v.Bytes;
  }
  static void Put(RegistryState state,string name,string value) { state.Values.RemoveAll(delegate(RegistryValueState v) { return SamePath(v.Name,name); }); state.Values.Add(new RegistryValueState { Name=name,Kind=RegistryValueKind.String,Text=value }); }
  static void PutNumber(RegistryState state,string name,int value) { state.Values.RemoveAll(delegate(RegistryValueState v) { return SamePath(v.Name,name); }); state.Values.Add(new RegistryValueState { Name=name,Kind=RegistryValueKind.DWord,Number=value }); }
  static void Transition(string hive,RegistryState expected,RegistryState desired) {
   var actual=Capture(hive,expected.Path);
   if(RegistryEqual(actual,desired)) return; // no write access required for an unchanged value
   Need(RegistryEqual(actual,expected),"registry changed concurrently; refused to overwrite it.");
   using(var b=Base(hive)) {
    if(!desired.Existed) b.DeleteSubKey(expected.Path,false); // exact product key only, never recursive
    else {
     using(var k=b.CreateSubKey(desired.Path)) {
      foreach(var v in expected.Values) if(Find(desired,v.Name)==null) k.DeleteValue(v.Name,false);
      foreach(var v in desired.Values) if(!ValuesEqual(v,Find(expected,v.Name))) {
       var now=Capture(hive,expected.Path);
       Need(ValuesEqual(Find(now,v.Name),Find(expected,v.Name)),"registry value changed concurrently; recovery retained.");
       k.SetValue(v.Name,Value(v),v.Kind);
       Need(ValuesEqual(Find(Capture(hive,expected.Path),v.Name),v),"registry write readback failed.");
      }
      if(!expected.Existed&&desired.Acl!=null) { var acl=new RegistrySecurity(); acl.SetSecurityDescriptorSddlForm(desired.Acl,AccessControlSections.Access); k.SetAccessControl(acl); }
      k.Flush();
     }
    }
   }
   Need(RegistryEqual(Capture(hive,expected.Path),desired),"registry result readback failed.");
  }
  static void Validate(MetadataState state,string root) {
   Need(SamePath(state.Root,root),"journal directory mismatch.");
   bool all=state.Hive=="LocalMachine";
   Need(SamePath(state.Desktop,Environment.GetFolderPath(all?Environment.SpecialFolder.CommonDesktopDirectory:Environment.SpecialFolder.DesktopDirectory)),"desktop scope mismatch.");
   Need(SamePath(state.Programs,Environment.GetFolderPath(all?Environment.SpecialFolder.CommonPrograms:Environment.SpecialFolder.Programs)),"start menu scope mismatch.");
   string folder=state.StartMenuFolder??"";
   Need(!Path.IsPathRooted(folder)&&Array.IndexOf(folder.Split('\\','/'),"..")<0,"invalid start menu folder.");
   var allowed=new List<string> { Path.Combine(state.Desktop,"In Line.lnk"),Path.Combine(state.Programs,"In Line.lnk"),Path.Combine(state.Programs,folder,"In Line.lnk") };
   foreach(var link in state.Shortcuts) Need(allowed.Exists(delegate(string p) { return SamePath(p,link.Path); }),"unknown shortcut in recovery journal.");
   foreach(var reg in state.Registry) Need((reg.Before.Path==Arp||reg.Before.Path==Vendor)&&reg.After.Path==reg.Before.Path,"unknown registry key in recovery journal.");
   foreach(var link in state.Shortcuts) {
    Need(link.Before==null?link.BeforeHash==null:Digest(link.Before)==link.BeforeHash,"shortcut recovery bytes are not verified.");
    Need(link.After==null?link.AfterHash==null:Digest(link.After)==link.AfterHash,"shortcut planned bytes are not verified.");
   }
  }
  public MetadataTransaction() { }
  void Parent(string path) {
   if(Directory.Exists(path)) return;
   Parent(Path.GetDirectoryName(path)); Guard.CheckPath(path); Directory.CreateDirectory(path); directories.Add(path);
  }
  public void Prepare(MetadataOptions options,string root,string operation) {
   Need(options.Publisher=="github","publisher changed; review fixed metadata scope first.");
   State=new MetadataState { Root=root,Hive=options.Hive,Desktop=options.Desktop,Programs=options.Programs,StartMenuFolder=options.StartMenuFolder };
   Validate(State,root);
   var paths=new List<string>();
   paths.Add(Path.Combine(State.Desktop,"In Line.lnk")); paths.Add(Path.Combine(State.Programs,"In Line.lnk"));
   string selected=Path.Combine(State.Programs,State.StartMenuFolder??"","In Line.lnk");
   if(!paths.Exists(delegate(string p) { return SamePath(p,selected); })) paths.Add(selected);
   foreach(string path in paths) {
    if(operation=="desktop"&&!SamePath(path,paths[0])) continue;
    bool requested=operation!="uninstall"&&((options.DesktopShortcut&&SamePath(path,paths[0]))||(options.StartMenu&&SamePath(path,selected)));
    bool exists=File.Exists(path); if(!exists&&!requested) continue;
    Guard.CheckPath(path);
    if(exists) {
     // Inspect first without a write. Re-read from the exclusive handle before use.
     var target=ShellLink.Target(File.ReadAllBytes(path));
     bool owned=SamePath(target,Path.Combine(root,"in-line.exe"));
     if(!owned) { Need(!requested,"required shortcut path belongs to another target; preserved."); continue; }
     if(operation=="uninstall"&&!options.RemoveShortcuts) continue;
    }
    Parent(Path.GetDirectoryName(path));
    var stream=Guard.Open(path,!exists); streams.Add(stream); created.Add(!exists);
    var change=new ShortcutChange { Path=path };
    State.Shortcuts.Add(change);
    if(exists) {
     change.Before=Read(stream); change.BeforeHash=Digest(change.Before);
     Need(SamePath(ShellLink.Target(change.Before),Path.Combine(root,"in-line.exe")),"shortcut target changed during identification.");
     change.Acl=stream.GetAccessControl().GetSecurityDescriptorSddlForm(AccessControlSections.Access);
     change.Created=File.GetCreationTimeUtc(path).ToFileTimeUtc(); change.Accessed=File.GetLastAccessTimeUtc(path).ToFileTimeUtc(); change.Written=File.GetLastWriteTimeUtc(path).ToFileTimeUtc();
    }
    if(operation!="uninstall") {
     change.After=exists?change.Before:ShellLink.Create(root,options.BundleId);
     change.AfterHash=Digest(change.After);
    }
   }
   if(operation=="desktop") return;
   var before=Capture(State.Hive,Arp); var after=Capture(State.Hive,Arp);
   var location=Find(before,"InstallLocation");
   if(location!=null) Need(SamePath(location.Text.Trim('"'),root),"registration belongs to another installation.");
   if(operation=="install") {
    after.Existed=true;
    Put(after,"MainBinaryName","in-line.exe"); Put(after,"DisplayName","In Line"); Put(after,"DisplayIcon","\""+Path.Combine(root,"in-line.exe")+"\"");
    Put(after,"DisplayVersion",options.Version); Put(after,"Publisher",options.Publisher); Put(after,"InstallLocation","\""+root+"\""); Put(after,"UninstallString","\""+Path.Combine(root,"uninstall.exe")+"\"");
    PutNumber(after,"NoModify",1); PutNumber(after,"NoRepair",1); PutNumber(after,"EstimatedSize",options.EstimatedSize);
   } else { after=new RegistryState { Path=Arp }; }
   State.Registry.Add(new RegistryChange { Before=before,After=after });
   before=Capture(State.Hive,Vendor); after=Capture(State.Hive,Vendor);
   if(operation=="install") { after.Existed=true; Put(after,"",root); }
   State.Registry.Add(new RegistryChange { Before=before,After=after });
  }
  public void Apply() {
   for(int i=0;i<streams.Count;i++) {
    var link=State.Shortcuts[i];
    if(link.After==null) Guard.DeleteOnClose(streams[i],true);
    else { Write(streams[i],link.After); Need(Guard.Hash(streams[i])==link.AfterHash,"shortcut write readback failed."); }
   }
   foreach(var reg in State.Registry) Transition(State.Hive,reg.Before,reg.After);
   completed=true;
  }
  public static MetadataTransaction Reopen(MetadataState state,string root) {
   if(state==null) return null;
   Validate(state,root); var transaction=new MetadataTransaction { State=state };
   try {
    foreach(var link in state.Shortcuts) {
     bool missing=!File.Exists(link.Path); transaction.Parent(Path.GetDirectoryName(link.Path));
     transaction.streams.Add(Guard.Open(link.Path,missing)); transaction.created.Add(missing);
    }
    return transaction;
   } catch { transaction.Dispose(); throw; }
  }
  public void Rollback() {
   // Refuse before changing any shortcut if another program/user replaced it.
   for(int i=0;i<streams.Count;i++) {
    var link=State.Shortcuts[i]; string hash=Guard.Hash(streams[i]);
    Need((created[i]&&link.After==null)||hash==link.BeforeHash||hash==link.AfterHash||(created[i]&&link.Before==null&&streams[i].Length==0),"shortcut changed concurrently; backup retained, no overwrite.");
   }
   // A partial registry write is restored value by value, only while it matches
   // either our captured baseline or our planned value. Unrelated values survive.
   for(int i=State.Registry.Count-1;i>=0;i--) {
    var reg=State.Registry[i]; var current=Capture(State.Hive,reg.Before.Path);
    if(RegistryEqual(current,reg.Before)) continue;
    if(!current.Existed) { Transition(State.Hive,current,reg.Before); continue; }
    foreach(var value in current.Values) Need(ValuesEqual(value,Find(reg.Before,value.Name))||ValuesEqual(value,Find(reg.After,value.Name)),"registry concurrently modified; rollback not complete.");
    foreach(var value in reg.Before.Values) Need(Find(current,value.Name)!=null,"registry value removed concurrently; rollback not complete.");
    Transition(State.Hive,current,reg.Before);
   }
   for(int i=0;i<streams.Count;i++) {
    var link=State.Shortcuts[i]; Guard.DeleteOnClose(streams[i],false);
    if(link.Before==null) Guard.DeleteOnClose(streams[i],true);
    else {
     Write(streams[i],link.Before); Need(Guard.Hash(streams[i])==link.BeforeHash,"restored shortcut bytes failed verification.");
     Guard.RestoreShortcutMetadata(streams[i],link,created[i]);
    }
   }
   completed=false;
  }
  public static void Verify(MetadataState state,string root) {
   if(state==null) return; Validate(state,root);
   foreach(var reg in state.Registry) Need(RegistryEqual(Capture(state.Hive,reg.After.Path),reg.After),"final registry readback changed; recovery retained.");
   foreach(var link in state.Shortcuts) Need(link.After==null?!File.Exists(link.Path):(File.Exists(link.Path)&&Digest(File.ReadAllBytes(link.Path))==link.AfterHash),"final shortcut readback failed; recovery retained.");
  }
  public void Dispose() {
   for(int i=0;i<streams.Count;i++) {
    // A failed recovery preflight may have opened an absent link with CREATE_NEW.
    // Remove that empty placeholder, but retain any original bytes already restored.
    if(!completed&&created[i]&&(State.Shortcuts[i].Before==null||Guard.Hash(streams[i])!=State.Shortcuts[i].BeforeHash)) Guard.DeleteOnClose(streams[i],true);
    streams[i].Dispose();
   }
   if(!completed) for(int i=directories.Count-1;i>=0;i--) if(Directory.Exists(directories[i])&&Directory.GetFileSystemEntries(directories[i]).Length==0) Directory.Delete(directories[i]);
  }
 }
 internal static class ShellLink {
  [ComImport,Guid("00021401-0000-0000-C000-000000000046")] class Link { }
  [StructLayout(LayoutKind.Sequential)] struct Key { public Guid Format; public uint Id; }
  [StructLayout(LayoutKind.Explicit,Size=24)] struct Variant { [FieldOffset(0)] public ushort Type; [FieldOffset(8)] public IntPtr Text; }
  [ComImport,Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"),InterfaceType(ComInterfaceType.InterfaceIsIUnknown)] interface Properties {
   void GetCount(out uint count); void GetAt(uint index,out Key key); void GetValue(ref Key key,out Variant value); void SetValue(ref Key key,ref Variant value); void Commit();
  }
  static object Invoke(object obj,string name,BindingFlags flags,params object[] args) { return obj.GetType().InvokeMember(name,flags,null,obj,args); }
  static byte[] WithLink(byte[] bytes,string root,string bundle,out string target) {
   string temp=Path.Combine(Path.GetTempPath(),"inline-link-"+Guid.NewGuid().ToString("N")+".lnk"); object shell=null,shortcut=null,propertyLink=null;
   try {
    if(bytes!=null) File.WriteAllBytes(temp,bytes);
    shell=Activator.CreateInstance(Type.GetTypeFromProgID("WScript.Shell",true)); shortcut=Invoke(shell,"CreateShortcut",BindingFlags.InvokeMethod,temp);
    if(bytes==null) {
     Invoke(shortcut,"TargetPath",BindingFlags.SetProperty,Path.Combine(root,"in-line.exe")); Invoke(shortcut,"WorkingDirectory",BindingFlags.SetProperty,root); Invoke(shortcut,"Save",BindingFlags.InvokeMethod);
     propertyLink=new Link(); ((IPersistFile)propertyLink).Load(temp,2);
     var key=new Key { Format=new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"),Id=5 }; var value=new Variant { Type=31,Text=Marshal.StringToCoTaskMemUni(bundle) };
     try { ((Properties)propertyLink).SetValue(ref key,ref value); ((Properties)propertyLink).Commit(); ((IPersistFile)propertyLink).Save(temp,true); } finally { Marshal.FreeCoTaskMem(value.Text); }
    }
    target=(string)Invoke(shortcut,"TargetPath",BindingFlags.GetProperty);
    return File.ReadAllBytes(temp);
   } finally {
    if(propertyLink!=null) Marshal.FinalReleaseComObject(propertyLink); if(shortcut!=null) Marshal.FinalReleaseComObject(shortcut); if(shell!=null) Marshal.FinalReleaseComObject(shell);
    if(File.Exists(temp)) File.Delete(temp);
   }
  }
  public static string Target(byte[] bytes) { string target; WithLink(bytes,null,null,out target); return target; }
  public static byte[] Create(string root,string bundle) { string target; var bytes=WithLink(null,root,bundle,out target); Guard.Require(String.Equals(Target(bytes),Path.Combine(root,"in-line.exe"),StringComparison.OrdinalIgnoreCase),"New shortcut target readback failed."); return bytes; }
 }
}
