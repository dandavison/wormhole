-- Wormhole Hammerspoon integration
-- Usage in init.lua:
--   local wormhole = require("wormhole")
--   hs.hotkey.bind({}, "f13", wormhole.select)
--   hs.hotkey.bind({ "cmd", "control" }, "left", wormhole.previous)
--   etc.

local M = {}

M.host = "http://wormhole:7117"

function M.select()
    local frontApp = hs.application.frontmostApplication()
    if frontApp and frontApp:name() == "Wormhole" then
        -- Already focused: toggle between current/available projects
        hs.eventtap.keyStroke({}, "tab")
    else
        hs.application.launchOrFocus("/Applications/Wormhole.app")
    end
end

function M.previous()
    hs.http.asyncGet(M.host .. "/previous-project/", nil, function() end)
end

function M.next()
    hs.http.asyncGet(M.host .. "/next-project/", nil, function() end)
end

function M.pin()
    hs.http.asyncPost(M.host .. "/pin/", "", nil, function() end)
end

function M.openProject(name)
    hs.http.asyncGet(M.host .. "/project/" .. name, nil, function() end)
end

function M.getOpenProjects()
    local projects = {}
    local handle = io.popen("wormhole list")
    if handle then
        for line in handle:lines() do
            local project = line:match("^%s*(.-)%s*$")
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

return M
