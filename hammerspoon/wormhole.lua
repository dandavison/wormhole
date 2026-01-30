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

-- Neighbor overlay state (must be declared before M.previous/M.next)
local neighborAlertId = nil
local neighborTap = nil
local neighborOverlayActive = false
local neighborDisplayOrder = nil  -- locked display order while overlay is shown
local neighborCurrentIdx = nil    -- current position in neighborDisplayOrder
local neighborEditorWindows = nil -- cached set of projects with editor windows
local refreshNeighborOverlay      -- forward declaration

-- Get set of project names that have editor windows open (Cursor/Code)
local function getEditorWindows()
    local projects = {}
    for _, appName in ipairs({ "Cursor", "Code" }) do
        local app = hs.application.find(appName)
        if app then
            for _, win in ipairs(app:allWindows()) do
                local title = win:title() or ""
                -- Extract project name from window title (typically "filename - projectname")
                local project = title:match(" %- ([^%-]+)$")
                if project then
                    projects[project] = true
                end
            end
        end
    end
    return projects
end

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

-- Check if target project has an editor window (using cached data if available)
local function hasEditorWindow(item)
    if not neighborEditorWindows then return true end -- assume yes if not cached
    if item.branch then
        -- For tasks, check both "repo:branch" and just the branch
        return neighborEditorWindows[item.name .. ":" .. item.branch]
            or neighborEditorWindows[item.branch]
            or neighborEditorWindows[item.name]
    else
        return neighborEditorWindows[item.name]
    end
end

function M.previous()
    local url = M.host .. "/project/previous"
    -- During overlay with cached state, check target without extra HTTP call
    if neighborOverlayActive and neighborDisplayOrder and neighborCurrentIdx and neighborEditorWindows then
        local n = #neighborDisplayOrder
        -- Moving left in display = previous
        local targetIdx = neighborCurrentIdx - 1
        if targetIdx < 1 then targetIdx = n end
        local target = neighborDisplayOrder[targetIdx]
        if target and not hasEditorWindow(target) then
            url = url .. "?skip-editor=true"
        end
        neighborCurrentIdx = targetIdx
    end
    hs.http.asyncGet(url, nil, function()
        if neighborOverlayActive then refreshNeighborOverlay() end
    end)
end

function M.next()
    local url = M.host .. "/project/next"
    -- During overlay with cached state, check target without extra HTTP call
    if neighborOverlayActive and neighborDisplayOrder and neighborCurrentIdx and neighborEditorWindows then
        local n = #neighborDisplayOrder
        -- Moving right in display = next
        local targetIdx = neighborCurrentIdx + 1
        if targetIdx > n then targetIdx = 1 end
        local target = neighborDisplayOrder[targetIdx]
        if target and not hasEditorWindow(target) then
            url = url .. "?skip-editor=true"
        end
        neighborCurrentIdx = targetIdx
    end
    hs.http.asyncGet(url, nil, function()
        if neighborOverlayActive then refreshNeighborOverlay() end
    end)
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

-- Helper to get unique identifier for a project/task
local function itemKey(item)
    if item.branch then
        return item.name .. ":" .. item.branch
    else
        return item.name
    end
end

-- Neighbor overlay (shows prev/next when ctrl+cmd held)
local function renderNeighborOverlay()
    hs.http.asyncGet(M.host .. "/project/neighbors", nil, function(status, body)
        if status ~= 200 or not neighborOverlayActive then return end
        local ok, data = pcall(hs.json.decode, body)
        if not ok or not data.ring then return end

        local ring = data.ring
        local n = #ring
        if n == 0 then return end

        local currentKey = ring[1] and itemKey(ring[1])

        -- Lock display order on first show, keep it fixed while overlay is visible
        -- Center current item, with "previous" items to the left
        if not neighborDisplayOrder then
            neighborDisplayOrder = {}
            local offset = n - math.floor(n / 2)
            for i = n, 1, -1 do
                local srcIdx = ((i - 1 + offset) % n) + 1
                table.insert(neighborDisplayOrder, ring[srcIdx])
            end
            -- Current is at center position
            neighborCurrentIdx = math.ceil(n / 2)
            -- Cache editor windows (fast local query, no network)
            neighborEditorWindows = getEditorWindows()
        end

        hs.timer.doAfter(0, function()
            if not neighborOverlayActive then return end

            local styledText = hs.styledtext.new("")
            for i, item in ipairs(neighborDisplayOrder) do
                if i > 1 then
                    styledText = styledText .. hs.styledtext.new("    ", { font = { size = 14 } })
                end

                local isCurrent = (itemKey(item) == currentKey)
                local dimColor = { white = 0.5, alpha = 0.8 }
                local brightColor = { white = 1, alpha = 1 }

                if item.branch then
                    -- Task: name(branch) format, horizontal
                    local nameColor = isCurrent and { white = 0.7, alpha = 1 } or { white = 0.4, alpha = 0.7 }
                    local branchColor = isCurrent and brightColor or dimColor
                    local nameFont = { size = 12 }
                    local branchFont = isCurrent and { size = 14, name = "Menlo-Bold" } or { size = 12 }

                    styledText = styledText .. hs.styledtext.new(item.name, { font = nameFont, color = nameColor })
                    styledText = styledText .. hs.styledtext.new("(", { font = nameFont, color = nameColor })
                    styledText = styledText .. hs.styledtext.new(item.branch, { font = branchFont, color = branchColor })
                    styledText = styledText .. hs.styledtext.new(")", { font = nameFont, color = nameColor })
                else
                    -- Single name for regular projects
                    local color = isCurrent and brightColor or dimColor
                    local font = isCurrent and { size = 14, name = "Menlo-Bold" } or { size = 12 }
                    styledText = styledText .. hs.styledtext.new(item.name, { font = font, color = color })
                end
            end

            if neighborAlertId then
                hs.alert.closeSpecific(neighborAlertId)
            end
            neighborAlertId = hs.alert.show(styledText, {
                fillColor = { white = 0.1, alpha = 0.9 },
                strokeColor = { white = 0.3, alpha = 1 },
                strokeWidth = 2,
                radius = 10,
                fadeInDuration = 0,
                fadeOutDuration = 0,
                atScreenEdge = 0
            }, "♾️")
        end)
    end)
end

local function showNeighborOverlay()
    if neighborOverlayActive then return end
    neighborOverlayActive = true
    renderNeighborOverlay()
end

refreshNeighborOverlay = function()
    if neighborOverlayActive then
        renderNeighborOverlay()
    end
end

local function hideNeighborOverlay()
    neighborOverlayActive = false
    neighborDisplayOrder = nil -- reset for next show
    neighborCurrentIdx = nil
    neighborEditorWindows = nil
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

local dashboardPreviousApp = nil

function M.focusDashboard()
    local frontApp = hs.application.frontmostApplication()
    local isIsland = frontApp and frontApp:name() == "Island"

    if isIsland and dashboardPreviousApp then
        dashboardPreviousApp:activate()
        dashboardPreviousApp = nil
        return
    end

    if not isIsland then
        dashboardPreviousApp = frontApp
    end

    local dashboardUrl = M.host .. "/dashboard"
    local script = [[
        (() => {
            const island = Application("Island");
            island.activate();
            const windows = island.windows();
            for (const win of windows) {
                const tabs = win.tabs();
                for (let i = 0; i < tabs.length; i++) {
                    if (tabs[i].url().includes("wormhole") && tabs[i].url().includes("/dashboard")) {
                        win.activeTabIndex = i + 1;
                        return true;
                    }
                }
            }
            if (windows.length > 0) {
                island.openLocation("]] .. dashboardUrl .. [[");
            }
            return false;
        })()
    ]]
    hs.osascript.javascript(script)
end

local dashboardTap = nil

function M.bindDashboardKey()
    local wasRightAlt = false
    local otherKeyPressed = false

    dashboardTap = hs.eventtap.new({
        hs.eventtap.event.types.flagsChanged,
        hs.eventtap.event.types.keyDown
    }, function(event)
        local eventType = event:getType()

        if eventType == hs.eventtap.event.types.keyDown then
            if wasRightAlt then
                otherKeyPressed = true
            end
            return false
        end

        local flags = event:getFlags()
        local rawFlags = event:getRawEventData().CGEventData.flags

        -- Right Alt: bit 0x40 in rawFlags indicates right-side modifier
        local isRightAlt = flags.alt and (rawFlags & 0x40) ~= 0

        if isRightAlt and not wasRightAlt then
            wasRightAlt = true
            otherKeyPressed = false
        elseif wasRightAlt and not flags.alt then
            if not otherKeyPressed then
                M.focusDashboard()
            end
            wasRightAlt = false
            otherKeyPressed = false
        end

        return false
    end)
    dashboardTap:start()
end

function M.bindKeys(keymap)
    M.bindSelect({ "cmd" }, "f13")
    hs.hotkey.bind({ "cmd", "control" }, "left", M.previous)
    hs.hotkey.bind({ "cmd", "control" }, "right", M.next)
    hs.hotkey.bind({ "cmd", "control" }, ".", M.pin)
    hs.hotkey.bind({ "cmd", "alt" }, "k", M.createHotkeyOverlay(keymap))
    M.bindProjectHotkeys(keymap)
    M.bindNeighborOverlay()
    M.bindDashboardKey()
end

return M
