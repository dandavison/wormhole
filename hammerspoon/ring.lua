-- Ring switcher: ctrl+cmd+left/right navigation with overlay
local M = {}

M.host = "http://wormhole:7117"

local alertId = nil
local tap = nil
local overlayActive = false
local displayOrder = nil
local currentIdx = nil
local editorWindows = nil

local function getEditorWindows()
    local projects = {}
    for _, appName in ipairs({ "Cursor", "Code" }) do
        local app = hs.application.find(appName)
        if app then
            for _, win in ipairs(app:allWindows()) do
                local title = win:title() or ""
                local project = title:match(" [%-–—] ([^%-–—]+)$")
                if project then
                    projects[project] = true
                else
                    projects[title] = true
                end
            end
        end
    end
    return projects
end

local function hasEditorWindow(item)
    if not editorWindows then return true end
    return editorWindows[item.project_key]
end

local function parseProjectKey(project_key)
    local name, branch = project_key:match("^([^:]+):(.+)$")
    if name then
        return name, branch
    else
        return project_key, nil
    end
end

local function render()
    hs.http.asyncGet(M.host .. "/project/neighbors?active=true", nil, function(status, body)
        if status ~= 200 or not overlayActive then return end
        local ok, data = pcall(hs.json.decode, body)
        if not ok or not data.ring then return end

        local ring = data.ring
        local n = #ring
        if n == 0 then return end

        local currentKey = ring[1] and ring[1].project_key

        if not displayOrder then
            displayOrder = {}
            local offset = n - math.floor(n / 2)
            for i = n, 1, -1 do
                local srcIdx = ((i - 1 + offset) % n) + 1
                table.insert(displayOrder, ring[srcIdx])
            end
            currentIdx = math.ceil(n / 2)
            editorWindows = getEditorWindows()
        end

        hs.timer.doAfter(0, function()
            if not overlayActive then return end

            local styledText = hs.styledtext.new("")
            for i, item in ipairs(displayOrder) do
                if i > 1 then
                    styledText = styledText .. hs.styledtext.new("    ", { font = { size = 14 } })
                end

                local isCurrent = (item.project_key == currentKey)
                local dimColor = { white = 0.5, alpha = 0.8 }
                local brightColor = { white = 1, alpha = 1 }

                local name, branch = parseProjectKey(item.project_key)
                if branch then
                    local nameColor = isCurrent and { white = 0.7, alpha = 1 } or { white = 0.4, alpha = 0.7 }
                    local branchColor = isCurrent and brightColor or dimColor
                    local nameFont = { size = 12 }
                    local branchFont = isCurrent and { size = 14, name = "Menlo-Bold" } or { size = 12 }

                    styledText = styledText .. hs.styledtext.new(name, { font = nameFont, color = nameColor })
                    styledText = styledText .. hs.styledtext.new("(", { font = nameFont, color = nameColor })
                    styledText = styledText .. hs.styledtext.new(branch, { font = branchFont, color = branchColor })
                    styledText = styledText .. hs.styledtext.new(")", { font = nameFont, color = nameColor })
                else
                    local color = isCurrent and brightColor or dimColor
                    local font = isCurrent and { size = 14, name = "Menlo-Bold" } or { size = 12 }
                    styledText = styledText .. hs.styledtext.new(name, { font = font, color = color })
                end
            end

            if alertId then
                hs.alert.closeSpecific(alertId)
            end
            alertId = hs.alert.show(styledText, {
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

local function show()
    if overlayActive then return end
    overlayActive = true
    render()
end

local function hide()
    overlayActive = false
    displayOrder = nil
    currentIdx = nil
    editorWindows = nil
    if alertId then
        hs.alert.closeSpecific(alertId)
        alertId = nil
    end
end

function M.previous()
    local url = M.host .. "/project/previous?active=true"
    if overlayActive and displayOrder and currentIdx and editorWindows then
        local n = #displayOrder
        local targetIdx = currentIdx - 1
        if targetIdx < 1 then targetIdx = n end
        local target = displayOrder[targetIdx]
        if target and not hasEditorWindow(target) then
            url = url .. "&skip-editor=true"
        end
        currentIdx = targetIdx
    end
    hs.http.asyncGet(url, nil, function()
        if overlayActive then render() end
    end)
end

function M.next()
    local url = M.host .. "/project/next?active=true"
    if overlayActive and displayOrder and currentIdx and editorWindows then
        local n = #displayOrder
        local targetIdx = currentIdx + 1
        if targetIdx > n then targetIdx = 1 end
        local target = displayOrder[targetIdx]
        if target and not hasEditorWindow(target) then
            url = url .. "&skip-editor=true"
        end
        currentIdx = targetIdx
    end
    hs.http.asyncGet(url, nil, function()
        if overlayActive then render() end
    end)
end

function M.bind()
    tap = hs.eventtap.new({ hs.eventtap.event.types.flagsChanged }, function(event)
        local flags = event:getFlags()
        if flags.ctrl and flags.cmd and not flags.alt and not flags.shift then
            show()
        else
            hide()
        end
        return false
    end)
    tap:start()
end

return M
