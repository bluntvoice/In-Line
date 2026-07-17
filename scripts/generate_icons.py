from pathlib import Path
from PIL import Image, ImageDraw

ROOT=Path(__file__).resolve().parents[1]
PUBLIC=ROOT/"public"
TAURI=ROOT/"src-tauri"/"icons"
TAURI.mkdir(parents=True,exist_ok=True)
(PUBLIC/"icons").mkdir(parents=True,exist_ok=True)
BLUE="#0B3A82";AMBER="#FFB000";WHITE="#F7F9FC"

def bell(size:int)->Image.Image:
    scale=4; s=size*scale
    image=Image.new("RGBA",(s,s),(0,0,0,0));d=ImageDraw.Draw(image)
    def line(points,fill,width):d.line([(int(x*s/256),int(y*s/256)) for x,y in points],fill=fill,width=max(1,int(width*s/256)),joint="curve")
    line([(66,179),(67,159),(77,137),(93,121),(111,112),(128,110),(147,114),(164,125),(177,142),(187,162),(190,179)],BLUE,30)
    line([(57,185),(199,185)],BLUE,18);line([(51,205),(205,205)],BLUE,20)
    line([(128,106),(128,79)],BLUE,15);line([(110,75),(146,75)],BLUE,15)
    line([(47,78),(28,59)],AMBER,13);line([(43,111),(18,103)],AMBER,13)
    line([(209,78),(228,59)],AMBER,13);line([(213,111),(238,103)],AMBER,13)
    line([(90,151),(98,139),(111,129),(129,124)],WHITE,9)
    return image.resize((size,size),Image.Resampling.LANCZOS)

def operation(kind:str,size=64)->Image.Image:
    scale=4;s=size*scale;image=Image.new("RGBA",(s,s),(0,0,0,0));d=ImageDraw.Draw(image)
    w=5*scale
    if kind=="copy":
        d.rounded_rectangle((22*scale,20*scale,52*scale,54*scale),radius=5*scale,outline=BLUE,width=w)
        d.line([(22*scale,45*scale),(16*scale,45*scale)],fill=BLUE,width=w)
        d.rounded_rectangle((11*scale,10*scale,42*scale,45*scale),radius=5*scale,outline=BLUE,width=w)
        d.rectangle((20*scale,17*scale,46*scale,49*scale),fill=(0,0,0,0))
        d.rounded_rectangle((22*scale,20*scale,52*scale,54*scale),radius=5*scale,outline=BLUE,width=w)
    else:
        pts=[(13,40),(32,21),(51,40),(32,21),(32,53)] if kind=="move-up" else [(13,24),(32,43),(51,24),(32,43),(32,11)]
        d.line([(x*scale,y*scale) for x,y in pts],fill=BLUE,width=6*scale,joint="curve")
    return image.resize((size,size),Image.Resampling.LANCZOS)

for size,name in [(32,"32x32.png"),(128,"128x128.png"),(256,"128x128@2x.png")]:
    bell(size).save(TAURI/name)
bell(1024).save(PUBLIC/"inline-mark.png")
for size in (16,20,32):bell(size).save(PUBLIC/"icons"/f"tray-{size}.png")
bell(256).save(TAURI/"icon.ico",format="ICO",sizes=[(v,v) for v in (16,24,32,48,64,128,256)])
for kind in ("copy","move-up","move-down"):operation(kind).save(PUBLIC/"icons"/f"{kind}.png")
