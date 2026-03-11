--- Lua Syntax Highlighting Test
--- A Neovim plugin configuration and game entity system.

local M = {}

-- Constants
local PLUGIN_NAME = "token-editor"
local VERSION = "0.4.0"
local MAX_BUFFER_SIZE = 1024 * 1024  -- 1MB
local DEFAULT_OPTS = {
    theme = "dark",
    line_numbers = true,
    tab_width = 4,
    wrap = false,
    diagnostics = {
        enabled = true,
        severity = { min = "WARN" },
    },
}

-- Pattern matching and string operations
local path_sep = package.config:sub(1, 1)
local is_windows = path_sep == "\\"
local home_dir = os.getenv("HOME") or os.getenv("USERPROFILE") or "~"

local function normalize_path(path)
    path = path:gsub("\\", "/")
    path = path:gsub("^~", home_dir)
    path = path:gsub("/+", "/")
    path = path:gsub("/$", "")
    return path
end

-- Metatables and OOP
local Vector2 = {}
Vector2.__index = Vector2

function Vector2.new(x, y)
    return setmetatable({ x = x or 0, y = y or 0 }, Vector2)
end

function Vector2:length()
    return math.sqrt(self.x * self.x + self.y * self.y)
end

function Vector2:normalized()
    local len = self:length()
    if len == 0 then return Vector2.new(0, 0) end
    return Vector2.new(self.x / len, self.y / len)
end

function Vector2:dot(other)
    return self.x * other.x + self.y * other.y
end

function Vector2.__add(a, b)
    return Vector2.new(a.x + b.x, a.y + b.y)
end

function Vector2.__sub(a, b)
    return Vector2.new(a.x - b.x, a.y - b.y)
end

function Vector2.__mul(a, b)
    if type(a) == "number" then
        return Vector2.new(a * b.x, a * b.y)
    elseif type(b) == "number" then
        return Vector2.new(a.x * b, a.y * b)
    end
    return Vector2.new(a.x * b.x, a.y * b.y)
end

function Vector2:__tostring()
    return string.format("Vec2(%.2f, %.2f)", self.x, self.y)
end

-- Entity Component System
local Entity = {}
Entity.__index = Entity

local next_id = 0

function Entity.new(name)
    next_id = next_id + 1
    return setmetatable({
        id = next_id,
        name = name or ("entity_" .. next_id),
        components = {},
        active = true,
        tags = {},
    }, Entity)
end

function Entity:add_component(name, data)
    self.components[name] = data
    return self
end

function Entity:get_component(name)
    return self.components[name]
end

function Entity:has_component(name)
    return self.components[name] ~= nil
end

function Entity:remove_component(name)
    self.components[name] = nil
    return self
end

function Entity:add_tag(tag)
    self.tags[tag] = true
    return self
end

function Entity:has_tag(tag)
    return self.tags[tag] == true
end

-- World manages entities and systems
local World = {}
World.__index = World

function World.new()
    return setmetatable({
        entities = {},
        systems = {},
        time = 0,
        delta_time = 0,
    }, World)
end

function World:spawn(name)
    local entity = Entity.new(name)
    self.entities[entity.id] = entity
    return entity
end

function World:despawn(id)
    self.entities[id] = nil
end

function World:query(...)
    local required = { ... }
    local results = {}

    for _, entity in pairs(self.entities) do
        if entity.active then
            local matches = true
            for _, component_name in ipairs(required) do
                if not entity:has_component(component_name) then
                    matches = false
                    break
                end
            end
            if matches then
                results[#results + 1] = entity
            end
        end
    end

    return results
end

function World:add_system(name, fn, priority)
    self.systems[#self.systems + 1] = {
        name = name,
        update = fn,
        priority = priority or 0,
        enabled = true,
    }
    table.sort(self.systems, function(a, b)
        return a.priority < b.priority
    end)
end

function World:update(dt)
    self.delta_time = dt
    self.time = self.time + dt

    for _, system in ipairs(self.systems) do
        if system.enabled then
            local ok, err = pcall(system.update, self, dt)
            if not ok then
                io.stderr:write(string.format(
                    "[%s] System '%s' error: %s\n",
                    os.date("%H:%M:%S"),
                    system.name,
                    tostring(err)
                ))
            end
        end
    end
end

-- Example systems using closures
local function create_movement_system()
    return function(world, dt)
        for _, entity in ipairs(world:query("position", "velocity")) do
            local pos = entity:get_component("position")
            local vel = entity:get_component("velocity")

            pos.x = pos.x + vel.x * dt
            pos.y = pos.y + vel.y * dt

            -- Boundary wrapping
            if pos.x > 800 then pos.x = 0
            elseif pos.x < 0 then pos.x = 800 end
            if pos.y > 600 then pos.y = 0
            elseif pos.y < 0 then pos.y = 600 end
        end
    end
end

local function create_collision_system(cell_size)
    cell_size = cell_size or 64

    return function(world, _dt)
        local grid = {}
        local entities = world:query("position", "collider")

        for _, entity in ipairs(entities) do
            local pos = entity:get_component("position")
            local cx = math.floor(pos.x / cell_size)
            local cy = math.floor(pos.y / cell_size)
            local key = cx .. "," .. cy

            grid[key] = grid[key] or {}
            grid[key][#grid[key] + 1] = entity
        end

        for _, cell in pairs(grid) do
            for i = 1, #cell do
                for j = i + 1, #cell do
                    local a_pos = cell[i]:get_component("position")
                    local b_pos = cell[j]:get_component("position")
                    local a_col = cell[i]:get_component("collider")
                    local b_col = cell[j]:get_component("collider")

                    local dx = a_pos.x - b_pos.x
                    local dy = a_pos.y - b_pos.y
                    local dist = math.sqrt(dx * dx + dy * dy)

                    if dist < a_col.radius + b_col.radius then
                        -- Collision detected
                        if a_col.on_collide then a_col.on_collide(cell[i], cell[j]) end
                        if b_col.on_collide then b_col.on_collide(cell[j], cell[i]) end
                    end
                end
            end
        end
    end
end

-- Coroutine-based animation
local function tween(from, to, duration, easing)
    easing = easing or function(t) return t end
    local elapsed = 0

    return coroutine.wrap(function()
        while elapsed < duration do
            local t = easing(elapsed / duration)
            local value = from + (to - from) * t
            elapsed = elapsed + coroutine.yield(value)
        end
        return to
    end)
end

-- Multi-line string / long string
local shader_source = [[
    #version 330 core
    in vec2 uv;
    out vec4 fragColor;
    uniform float time;

    void main() {
        vec3 col = 0.5 + 0.5 * cos(time + uv.xyx + vec3(0, 2, 4));
        fragColor = vec4(col, 1.0);
    }
]]

-- Module setup function (Neovim plugin pattern)
function M.setup(opts)
    opts = vim.tbl_deep_extend("force", DEFAULT_OPTS, opts or {})

    vim.api.nvim_create_autocmd("BufEnter", {
        pattern = "*",
        callback = function(args)
            local bufnr = args.buf
            local filename = vim.api.nvim_buf_get_name(bufnr)

            if filename:match("%.token$") then
                vim.bo[bufnr].filetype = "token"
                vim.bo[bufnr].tabstop = opts.tab_width
            end
        end,
        desc = "Set up token filetype",
    })

    vim.api.nvim_create_user_command("TokenInfo", function()
        print(string.format("%s v%s", PLUGIN_NAME, VERSION))
    end, { desc = "Show Token Editor info" })
end

return M
