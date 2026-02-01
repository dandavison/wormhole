-- Ring switcher: ctrl+cmd+left/right navigation with overlay
-- Design: fetch once on show, navigate locally, switch on commit
local M = {}

M.host = "http://wormhole:7117"

local alertId = nil
local tap = nil
local overlayActive = false
local ring = nil           -- array of {project_key: string}
local currentIdx = nil     -- 1-based index of current selection
local startIdx = nil       -- index when overlay opened (to detect change)

local function parseProjectKey(project_key)
    local name, branch = project_key:match("^([^:]+):(.+)$")
    if name then
        return name, branch
    else
        return project_key, nil
    end
end

local function render()
    if not ring or #ring == 0 then return end

    local styledText = hs.styledtext.new("")
    for i, item in ipairs(ring) do
        if i > 1 then
            styledText = styledText .. hs.styledtext.new("    ", { font = { size = 14 } })
        end

        local isCurrent = (i == currentIdx)
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
end

local function show()
    if overlayActive then return end
    overlayActive = true

    -- Fetch active projects once
    hs.http.asyncGet(M.host .. "/project/neighbors?active=true", nil, function(status, body)
        if status ~= 200 or not overlayActive then return end
        local ok, data = pcall(hs.json.decode, body)
        if not ok or not data.ring then return end

        ring = data.ring
        if #ring == 0 then return end

        -- Current project is first in ring (server returns current at front)
        currentIdx = 1
        startIdx = 1
        render()
    end)
end

local function hide()
    if not overlayActive then return end

    -- If selection changed, switch to selected project
    if ring and currentIdx and startIdx and currentIdx ~= startIdx then
        local selected = ring[currentIdx]
        if selected then
            local url = M.host .. "/project/switch/" .. hs.http.encodeForQuery(selected.project_key)
            hs.http.asyncGet(url, nil, function() end)
        end
    end

    overlayActive = false
    ring = nil
    currentIdx = nil
    startIdx = nil
    if alertId then
        hs.alert.closeSpecific(alertId)
        alertId = nil
    end
end

function M.previous()
    if not overlayActive or not ring or #ring == 0 then
        -- No overlay: just do server call directly
        hs.http.asyncGet(M.host .. "/project/previous?active=true", nil, function() end)
        return
    end
    -- Navigate locally
    currentIdx = currentIdx - 1
    if currentIdx < 1 then currentIdx = #ring end
    render()
end

function M.next()
    if not overlayActive or not ring or #ring == 0 then
        -- No overlay: just do server call directly
        hs.http.asyncGet(M.host .. "/project/next?active=true", nil, function() end)
        return
    end
    -- Navigate locally
    currentIdx = currentIdx + 1
    if currentIdx > #ring then currentIdx = 1 end
    render()
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
