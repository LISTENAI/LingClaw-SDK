local previous_second
local function draw()
    screen.begin(0x142534)
    local now = clock.localtime()
    screen.text("LingClaw 时钟", 20, 24, 0xFFFFFF)
    if now then
        screen.text(string.format("%02d:%02d:%02d", now.hour, now.min, now.sec),
            20, 70, 0x55DDCC)
        screen.text(string.format("%04d-%02d-%02d", now.year, now.month, now.day),
            20, 106, 0xFFFFFF)
    else
        screen.text("等待校时", 20, 70, 0x55DDCC)
    end
    screen.present()
end
function on_start() draw() end
function on_tick(dt_ms)
    local second = clock.now()
    if second ~= previous_second then
        previous_second = second
        draw()
    end
end
