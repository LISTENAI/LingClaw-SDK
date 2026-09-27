local clicks = 0
local function draw()
    screen.begin(0x142534)
    screen.text("LingClaw", 20, 20, 0xFFFFFF)
    screen.text("点击计数: " .. clicks, 20, 60, 0x55DDCC)
    screen.rect(20, 110, 180, 4, 0x55DDCC)
    screen.present()
end
function on_start() draw() end
function on_tick(dt_ms) end
function on_button_click(button_id)
    if button_id == "function" then
        clicks = clicks + 1
        draw()
    end
end
