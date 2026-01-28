-- Wormhole Hammerspoon integration
-- Usage in init.lua:
--   local wormhole = require("wormhole")
--   wormhole.bindSelect({ "cmd" }, "f13")
--   hs.hotkey.bind({ "cmd", "control" }, "left", wormhole.previous)
--   etc.

local M = {}

M.host = "http://wormhole:7117"
M.selectRepeatInterval = 0.08 -- seconds between cycles when holding key
M.selectDebounce = 0.02       -- minimum seconds between down arrows

local selectTimer = nil
local selectTap = nil
local selectActive = false
local selectReverse = false
local lastMoveTime = 0

local function sendMove()
    local now = hs.timer.secondsSinceEpoch()
    if now - lastMoveTime < M.selectDebounce then return end
    lastMoveTime = now
    local key = selectReverse and "up" or "down"
    local down = hs.eventtap.event.newKeyEvent({}, key, true)
    local up = hs.eventtap.event.newKeyEvent({}, key, false)
    down:post()
    up:post()
end

local function stopSelect()
    selectActive = false
    local t = selectTimer
    selectTimer = nil
    if t then t:stop() end
end

local function startSelect(reverse)
    if selectActive then
        selectReverse = reverse
        return
    end
    selectActive = true
    selectReverse = reverse
    local frontApp = hs.application.frontmostApplication()
    if not (frontApp and frontApp:name() == "Wormhole") then
        hs.application.launchOrFocus("/Applications/Wormhole.app")
    end
    selectTimer = hs.timer.doEvery(M.selectRepeatInterval, function()
        if not selectTimer or not selectActive then return end
        local frontApp = hs.application.frontmostApplication()
        if frontApp and frontApp:name() == "Wormhole" then
            sendMove()
        else
            stopSelect()
        end
    end)
end

function M.bindSelect(mods, key)
    local keyCode = hs.keycodes.map[key]
    local wantCmd = false
    for _, m in ipairs(mods) do
        if m == "cmd" then wantCmd = true end
    end

    selectTap = hs.eventtap.new({ hs.eventtap.event.types.keyDown, hs.eventtap.event.types.keyUp }, function(event)
        if event:getKeyCode() ~= keyCode then return false end

        if event:getType() == hs.eventtap.event.types.keyDown then
            local flags = event:getFlags()
            if wantCmd and not flags.cmd then return false end
            startSelect(flags.shift)
            return true
        else
            if selectActive then
                stopSelect()
                return true
            end
            return false
        end
    end)
    selectTap:start()
end

function M.select()
    local frontApp = hs.application.frontmostApplication()
    if frontApp and frontApp:name() == "Wormhole" then
        hs.eventtap.keyStroke({}, "down")
    else
        hs.application.launchOrFocus("/Applications/Wormhole.app")
    end
end

function M.previous()
    hs.http.asyncGet(M.host .. "/project/previous", nil, function() end)
end

function M.next()
    hs.http.asyncGet(M.host .. "/project/next", nil, function() end)
end

function M.pin()
    hs.http.asyncPost(M.host .. "/project/pin", "", nil, function() end)
end

function M.openProject(name)
    hs.http.asyncGet(M.host .. "/project/switch/" .. name, nil, function() end)
end

function M.getOpenProjects()
    local projects = {}
    local handle = io.popen("wormhole project list")
    if handle then
        for line in handle:lines() do
            local project = line:match("^(%S+)")
            if project and project ~= "" then
                projects[project] = true
            end
        end
        handle:close()
    end
    return projects
end

-- Hotkey overlay showing project keybindings
-- keymap: table of {index -> project_name}
function M.createHotkeyOverlay(keymap)
    local alertId = nil

    return function()
        local openProjects = M.getOpenProjects()

        local lines = {}
        for i = 1, 9 do
            local repo = keymap[i]
            if repo then
                local isOpen = openProjects[repo]
                local line = string.format("%d    %s", i, repo)
                table.insert(lines, { text = line, available = isOpen })
            end
        end
        if keymap[0] then
            local isOpen = openProjects[keymap[0]]
            local line = string.format("0    %s", keymap[0])
            table.insert(lines, { text = line, available = isOpen })
        end

        if alertId then
            hs.alert.closeSpecific(alertId)
            alertId = nil
        else
            local styledText = hs.styledtext.new("")

            for i, lineData in ipairs(lines) do
                local color = lineData.available and { white = 1, alpha = 1 } or { white = 0.5, alpha = 0.7 }
                local text = lineData.text .. (i < #lines and "\n" or "")
                local styledLine = hs.styledtext.new(text, {
                    font = { size = 14 },
                    color = color
                })
                styledText = styledText .. styledLine
            end

            alertId = hs.alert.show(styledText, {
                fillColor = { white = 0.1, alpha = 0.9 },
                strokeColor = { white = 0.3, alpha = 1 },
                strokeWidth = 2,
                radius = 10,
                fadeInDuration = 0.15,
                fadeOutDuration = 0.15,
                atScreenEdge = 0
            }, "♾️")
        end
    end
end

-- Bind cmd+0-9 to open projects based on keymap
-- keymap: table of {index -> project_name}
function M.bindProjectHotkeys(keymap)
    for i = 0, 9 do
        hs.hotkey.bind({ "cmd" }, tostring(i), function()
            local repo = keymap[i]
            if repo then
                M.openProject(repo)
            end
        end)
    end
end

-- Neighbor overlay (shows prev/next when ctrl+cmd held)
local neighborAlertId = nil
local neighborTap = nil

local function showNeighborOverlay()
    if neighborAlertId then return end
    hs.http.asyncGet(M.host .. "/project/neighbors", nil, function(status, body)
        if status ~= 200 or neighborAlertId then return end
        local ok, data = pcall(hs.json.decode, body)
        if not ok then return end

        local prev = data.previous or "—"
        local curr = data.current or "—"
        local next = data.next or "—"

        local styledText = hs.styledtext.new(prev .. " ←", {
            font = { size = 14 },
            color = { white = 0.6, alpha = 1 }
        }) .. hs.styledtext.new("       " .. curr .. "       ", {
            font = { size = 16, name = "Menlo-Bold" },
            color = { white = 1, alpha = 1 }
        }) .. hs.styledtext.new("→ " .. next, {
            font = { size = 14 },
            color = { white = 0.6, alpha = 1 }
        })

        neighborAlertId = hs.alert.show(styledText, {
            fillColor = { white = 0.1, alpha = 0.9 },
            strokeColor = { white = 0.3, alpha = 1 },
            strokeWidth = 2,
            radius = 10,
            fadeInDuration = 0.1,
            fadeOutDuration = 0.1,
            atScreenEdge = 0
        }, "♾️")
    end)
end

local function hideNeighborOverlay()
    if neighborAlertId then
        hs.alert.closeSpecific(neighborAlertId)
        neighborAlertId = nil
    end
end

function M.bindNeighborOverlay()
    neighborTap = hs.eventtap.new({ hs.eventtap.event.types.flagsChanged }, function(event)
        local flags = event:getFlags()
        if flags.ctrl and flags.cmd and not flags.alt and not flags.shift then
            showNeighborOverlay()
        else
            hideNeighborOverlay()
        end
        return false
    end)
    neighborTap:start()
end

function M.bindKeys(keymap)
    M.bindSelect({ "cmd" }, "f13")
    hs.hotkey.bind({ "cmd", "control" }, "left", M.previous)
    hs.hotkey.bind({ "cmd", "control" }, "right", M.next)
    hs.hotkey.bind({ "cmd", "control" }, ".", M.pin)
    hs.hotkey.bind({ "cmd", "alt" }, "k", M.createHotkeyOverlay(keymap))
    M.bindProjectHotkeys(keymap)
    M.bindNeighborOverlay()
end

return M
