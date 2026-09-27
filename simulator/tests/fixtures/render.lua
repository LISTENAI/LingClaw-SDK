function on_tick(dt) end
function on_start()
 screen.begin(0x123456)
 screen.rect(0,0,240,8,0xABCDEF)
 screen.rect(10,30,100,60,0x987654)
 screen.rect(50,50,100,60,0xCAFEFE)
 screen.text("你好 LingClaw AVATAR",4,15,0xFFFFFF)
 screen.text("wrap text abcdefghijklmn0123456789",160,110,0x88EEAA)
 screen.text("line one\n第二行",12,170,0xFFFFFF)
 screen.text("edge",222,224,0xCC8844)
 screen.present()
end
