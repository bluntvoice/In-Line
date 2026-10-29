// Installer-only, finite transaction. No process termination or business storage APIs.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Security.Principal;
using System.Security.AccessControl;
using System.Text;
using System.Xml.Serialization;
using Microsoft.Win32.SafeHandles;

namespace InLine.Installer {
 public sealed class Entry { public string Name; public bool Existed; public string Before; public string After; public string Acl; public long Created, Accessed, Written; }
 public sealed class Journal { public List<Entry> Files = new List<Entry>(); }
 public sealed class Holder { public int Pid; public long Created; public string Path; public string User; public bool IdentityConfirmed; }
 public sealed class Result { public int Code; public string Message; public bool RolledBack; public List<Holder> Holders = new List<Holder>(); }
 public static class Guard {
  public static readonly string[] Names = {"in-line.exe", "in-line-mcp.exe", "in-line-cli.exe", "WebView2Loader.dll", "uninstall.exe"};
  const string Transaction = ".in-line-install-transaction";
  [StructLayout(LayoutKind.Sequential)] struct FileTime { public uint Low, High; public long Value { get { return ((long)High << 32) | Low; } } }
  [StructLayout(LayoutKind.Sequential)] struct FileInfo { public uint Attributes; public FileTime Creation, Access, Write; public uint Volume, SizeHigh, SizeLow, Links, IndexHigh, IndexLow; }
  [StructLayout(LayoutKind.Sequential)] struct UniqueProcess { public uint Pid; public FileTime Start; }
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] struct ProcessInfo {
   public UniqueProcess Process;
   [MarshalAs(UnmanagedType.ByValTStr, SizeConst=256)] public string App;
   [MarshalAs(UnmanagedType.ByValTStr, SizeConst=64)] public string Service;
   public uint Type, Status, Session; [MarshalAs(UnmanagedType.Bool)] public bool Restartable;
  }
  [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern SafeFileHandle CreateFile(string p, uint access, uint share, IntPtr security, uint mode, uint flags, IntPtr template);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool GetFileInformationByHandle(SafeFileHandle h, out FileInfo i);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool SetFileTime(SafeFileHandle h, ref FileTime c, ref FileTime a, ref FileTime w);
  [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern uint GetFinalPathNameByHandle(SafeFileHandle h, StringBuilder b, uint n, uint f);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool SetFileInformationByHandle(SafeFileHandle h, int type, ref int data, uint length);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool GetProcessTimes(IntPtr h, out FileTime c, out FileTime e, out FileTime k, out FileTime u);
  [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
  [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool QueryFullProcessImageName(IntPtr h, int flags, StringBuilder b, ref int n);
  [DllImport("advapi32.dll", SetLastError=true)] static extern bool OpenProcessToken(IntPtr h, uint access, out IntPtr token);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  [DllImport("rstrtmgr.dll", CharSet=CharSet.Unicode)] static extern int RmStartSession(out uint session, int flags, StringBuilder key);
  [DllImport("rstrtmgr.dll", CharSet=CharSet.Unicode)] static extern int RmRegisterResources(uint session, uint count, string[] paths, uint apps, IntPtr app, uint services, IntPtr service);
  [DllImport("rstrtmgr.dll")] static extern int RmGetList(uint session, out uint needed, ref uint count, [In,Out] ProcessInfo[] list, ref uint reason);
  [DllImport("rstrtmgr.dll")] static extern int RmEndSession(uint session);

  static void Require(bool condition, string message) { if (!condition) throw new IOException(message); }
  static string Hash(Stream s) { s.Position=0; using(var hash=SHA256.Create()) return BitConverter.ToString(hash.ComputeHash(s)).Replace("-", ""); }
  static string HashFile(string p) { using(var s=File.OpenRead(p)) return Hash(s); }
  static void CheckPath(string path) {
   string p=Path.GetFullPath(path);
   Require(!p.StartsWith(@"\\"), "Network paths are not supported.");
   for(string next=p; next!=null; next=Path.GetDirectoryName(next)) {
    if(File.Exists(next)||Directory.Exists(next)) Require((File.GetAttributes(next)&FileAttributes.ReparsePoint)==0, "Reparse points are not supported.");
   }
  }
  static string Root(string path) {
   string p=Path.GetFullPath(path).TrimEnd(Path.DirectorySeparatorChar);
   Require(Path.IsPathRooted(path) && p.Length>Path.GetPathRoot(p).Length+2, "A specific absolute installation directory is required.");
   CheckPath(p); Require(new DriveInfo(Path.GetPathRoot(p)).DriveFormat=="NTFS", "A local NTFS installation directory is required.");
   return p;
  }
  static FileStream Open(string p, bool create) {
   CheckPath(p);
   var h=CreateFile(p, create?0xC0070000u:0xC0010000u, 0, IntPtr.Zero, create?1u:3u, 0x80, IntPtr.Zero); // READ|WRITE|DELETE; WRITE_DAC only for newly created recovery files
   if(h.IsInvalid) { int code=Marshal.GetLastWin32Error(); h.Dispose(); throw new IOException("Cannot exclusively open target file (Windows error "+code+"): "+Path.GetFileName(p)); }
   try {
    FileInfo i; Require(GetFileInformationByHandle(h,out i) && i.Links==1, "Unverifiable or hard-linked target file.");
    var name=new StringBuilder(32768); uint size=GetFinalPathNameByHandle(h,name,32768,0);
    Require(size>0 && size<32768 && String.Equals(name.ToString().Replace(@"\\?\", ""),Path.GetFullPath(p),StringComparison.OrdinalIgnoreCase), "Target file path changed.");
    return new FileStream(h,FileAccess.ReadWrite);
   } catch { h.Dispose(); throw; }
  }
  static void DeleteOnClose(FileStream s, bool delete) { int value=delete?1:0; Require(SetFileInformationByHandle(s.SafeFileHandle,4,ref value,4),"Cannot safely mark/unmark target deletion."); }
  static void Write(FileStream target, string source) {
   target.Position=0; using(var input=File.OpenRead(source)) input.CopyTo(target);
   target.SetLength(target.Position); target.Flush(true);
  }
  static FileTime Time(long value) { return new FileTime { Low=(uint)value, High=(uint)((ulong)value>>32) }; }
  static void Restore(FileStream target, string folder, Entry e, bool restoreAcl) {
   Write(target,Path.Combine(folder,e.Name)); Require(Hash(target)==e.Before,"Restored file verification failed.");
   if(restoreAcl) {
    var security=new FileSecurity(); security.SetSecurityDescriptorSddlForm(e.Acl,AccessControlSections.Access); target.SetAccessControl(security);
    Require(target.GetAccessControl().GetSecurityDescriptorSddlForm(AccessControlSections.Access)==e.Acl,"Restored access descriptor could not be verified.");
   }
   var c=Time(e.Created); var a=Time(e.Accessed); var w=Time(e.Written);
   Require(SetFileTime(target.SafeFileHandle,ref c,ref a,ref w),"Cannot restore file timestamps.");
  }
  static void Save(string folder, Journal journal) {
   using(var s=new FileStream(Path.Combine(folder,"journal.xml"),FileMode.CreateNew,FileAccess.Write,FileShare.None)) { new XmlSerializer(typeof(Journal)).Serialize(s,journal); s.Flush(true); }
  }
  static Journal Load(string folder) {
   CheckPath(folder); Journal j;
   using(var s=File.OpenRead(Path.Combine(folder,"journal.xml"))) j=(Journal)new XmlSerializer(typeof(Journal)).Deserialize(s);
   Require(j.Files.Count==Names.Length,"Invalid transaction file list.");
   for(int i=0;i<Names.Length;i++) Require(j.Files[i].Name==Names[i],"Invalid transaction filename.");
   foreach(var e in j.Files) if(e.Existed) {
    Require(HashFile(Path.Combine(folder,e.Name))==e.Before,"Rollback backup verification failed.");
    Require(!String.IsNullOrEmpty(e.Acl),"Missing rollback access descriptor.");
    new FileSecurity().SetSecurityDescriptorSddlForm(e.Acl,AccessControlSections.Access);
   }
   return j;
  }
  static void Clean(string folder) {
   foreach(string p in Directory.GetFileSystemEntries(folder)) Require(Array.IndexOf(Names,Path.GetFileName(p))>=0 || Path.GetFileName(p)=="journal.xml", "Unknown recovery file; retained for inspection.");
   foreach(string name in Names) { string p=Path.Combine(folder,name); if(File.Exists(p)) File.Delete(p); }
   File.Delete(Path.Combine(folder,"journal.xml")); Directory.Delete(folder); // never recursive
  }
  public static List<Holder> Inspect(string root) {
   var holders=new List<Holder>(); uint session;
   int error=RmStartSession(out session,0,new StringBuilder(33));
   if(error!=0) { holders.Add(new Holder { Path="File-holder inspection unavailable ("+error+")" }); return holders; }
   try {
    var files=new List<string>(); foreach(string name in Names) if(File.Exists(Path.Combine(root,name))) files.Add(Path.Combine(root,name));
    if(files.Count==0) return holders;
    error=RmRegisterResources(session,(uint)files.Count,files.ToArray(),0,IntPtr.Zero,0,IntPtr.Zero);
    Require(error==0,"File-holder registration failed."); uint needed,count=0,reason=0;
    error=RmGetList(session,out needed,ref count,null,ref reason);
    if(error==0) return holders; Require(error==234 && needed<=1024,"File-holder list unavailable.");
    var list=new ProcessInfo[needed]; count=needed; error=RmGetList(session,out needed,ref count,list,ref reason);
    Require(error==0,"File-holder list changed; retry after safely closing the application.");
    for(int i=0;i<count;i++) {
     if(list[i].Process.Pid==(uint)Process.GetCurrentProcess().Id) continue;
     var info=list[i]; var record=new Holder { Pid=(int)info.Process.Pid, Created=info.Process.Start.Value }; holders.Add(record);
     IntPtr h=OpenProcess(0x1000,false,info.Process.Pid); if(h==IntPtr.Zero) continue;
     try {
      FileTime c,e,k,u; var path=new StringBuilder(32768); int n=32768; IntPtr token;
      if(!GetProcessTimes(h,out c,out e,out k,out u) || c.Value!=record.Created || !QueryFullProcessImageName(h,0,path,ref n)) continue;
      record.Path=path.ToString();
      if(OpenProcessToken(h,8,out token)) { try { using(var identity=new WindowsIdentity(token)) record.User=identity.User.Value; } finally { CloseHandle(token); } }
      record.IdentityConfirmed=record.User!=null;
     } finally { CloseHandle(h); }
    }
   } catch { holders.Add(new Holder { Path="File-holder identity could not be confirmed; no process action taken." }); }
   finally { RmEndSession(session); }
   return holders;
  }
  // failAfter is used by isolated fault-injection tests only; the packaged wrapper always passes -1.
  public static Result Run(string directory, string payload, string operation, int failAfter) {
   var result=new Result(); var streams=new List<FileStream>(); var created=new List<bool>(); FileStream serial=null; SafeFileHandle directoryHandle=null;
   string root=null,folder=null; Journal journal=null; bool mutated=false,prepared=false,success=false,ownedFolder=false;
   try {
    Require(operation=="install"||operation=="uninstall"||operation=="rollback"||operation=="finalize","Invalid installer operation.");
    root=Root(directory); Directory.CreateDirectory(root); folder=Path.Combine(root,Transaction);
    directoryHandle=CreateFile(root,0x80,3,IntPtr.Zero,3,0x02000000,IntPtr.Zero);
    Require(!directoryHandle.IsInvalid,"Cannot pin installation directory identity.");
    string lockPath=Path.Combine(root,".in-line-install.lock"); serial=Open(lockPath,!File.Exists(lockPath));
    Require(serial.Length==0,"Unrecognized installer lock; retained for inspection."); DeleteOnClose(serial,true);
    if(operation=="finalize") {
     Require(Directory.Exists(folder),"Missing transaction journal; cannot confirm completion.");
     if(Directory.Exists(folder)) {
      var complete=Load(folder);
      foreach(var e in complete.Files) Require(e.After==null?!File.Exists(Path.Combine(root,e.Name)):HashFile(Path.Combine(root,e.Name))==e.After,"Installed group changed; recovery backups retained.");
      Clean(folder);
     }
     success=true; return result;
    }
    bool restore=operation=="rollback";
    if(restore) journal=Load(folder);
    else {
     Require(!Directory.Exists(folder),"An incomplete installer transaction exists. Stop and inspect recovery backups; do not launch or overwrite a mixed installation.");
     journal=new Journal();
     foreach(string name in Names) {
      string source=payload==null?null:Path.Combine(payload,name);
      if(operation=="install") { CheckPath(source); Require(File.Exists(source),"Incomplete installation payload: "+name); }
      journal.Files.Add(new Entry { Name=name, Existed=File.Exists(Path.Combine(root,name)), After=operation=="install"?HashFile(source):null });
     }
    }
    // Acquire every target before changing any existing byte. A holder appearing
    // after inspection is blocked by the OS sharing mode, not a PID/name guess.
    foreach(var e in journal.Files) {
     string p=Path.Combine(root,e.Name); bool missing=!File.Exists(p); created.Add(missing); streams.Add(Open(p,missing));
    }
    if(restore) {
     for(int i=0;i<Names.Length;i++) {
      var e=journal.Files[i]; string actual=Hash(streams[i]);
      Require((e.After==null&&created[i])||actual==e.After||actual==e.Before,"Target changed since transaction; rollback refused.");
     }
     // Release new-group allocation before restoring differently sized old files.
     // All handles and verified backups are retained throughout this operation.
     mutated=true;
     foreach(var stream in streams) { stream.SetLength(0); stream.Flush(true); }
    } else {
     Directory.CreateDirectory(folder);
     ownedFolder=true;
     for(int i=0;i<Names.Length;i++) {
      var e=journal.Files[i]; if(!e.Existed) continue;
      FileInfo info; Require(GetFileInformationByHandle(streams[i].SafeFileHandle,out info),"Cannot snapshot file metadata.");
      e.Created=info.Creation.Value; e.Accessed=info.Access.Value; e.Written=info.Write.Value;
      e.Acl=streams[i].GetAccessControl().GetSecurityDescriptorSddlForm(AccessControlSections.Access);
      e.Before=Hash(streams[i]); streams[i].Position=0;
      using(var backup=new FileStream(Path.Combine(folder,e.Name),FileMode.CreateNew,FileAccess.Write,FileShare.None)) { streams[i].CopyTo(backup); backup.Flush(true); }
      Require(HashFile(Path.Combine(folder,e.Name))==e.Before,"Backup verification failed.");
     }
     Save(folder,journal); prepared=true;
    }
    for(int i=0;i<Names.Length;i++) {
     mutated=true; var e=journal.Files[i];
     if(restore) { if(e.Existed) Restore(streams[i],folder,e,created[i]); else DeleteOnClose(streams[i],true); }
     else if(operation=="install") { Write(streams[i],Path.Combine(payload,e.Name)); Require(Hash(streams[i])==e.After,"Installed file verification failed."); }
     else DeleteOnClose(streams[i],true);
     if(i+1==failAfter) throw new IOException("Isolated injected file transaction failure.");
    }
    if(restore) Clean(folder);
    success=true; result.Message="Verified complete "+operation+" file group."; return result;
   } catch(Exception error) {
    result.Code=1; result.Message=error.Message+" Installation stopped; no process was terminated.";
    if(operation=="rollback"&&mutated) { result.Code=2; result.Message+=" Recovery incomplete; retain backups and do not launch this installation."; }
    if(mutated&&prepared) {
     try {
      foreach(var stream in streams) DeleteOnClose(stream,false);
      if(operation=="install") foreach(var stream in streams) { stream.SetLength(0); stream.Flush(true); }
      for(int i=0;i<streams.Count;i++) {
       var e=journal.Files[i];
       if(e.Existed&&operation=="uninstall") Require(Hash(streams[i])==e.Before,"Uninstall rollback bytes changed.");
       else if(e.Existed) Restore(streams[i],folder,e,false);
       else DeleteOnClose(streams[i],true);
      }
      Clean(folder); result.RolledBack=true;
     } catch { result.Code=2; result.Message+=" Rollback could not be verified. Keep recovery backups and stop using this installation; manual recovery is required."; }
    }
    if(!mutated&&ownedFolder) { try { Clean(folder); } catch { result.Message+=" Preparation backups retained for inspection."; } }
    if(root!=null) result.Holders=Inspect(root);
    return result;
   } finally {
    // Empty files created solely during a failed preflight must not survive.
    if(!success&&!mutated) for(int i=0;i<streams.Count;i++) if(created[i]) DeleteOnClose(streams[i],true);
    foreach(var s in streams) s.Dispose(); if(serial!=null) serial.Dispose(); if(directoryHandle!=null) directoryHandle.Dispose();
   }
  }
 }
}
