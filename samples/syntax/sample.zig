/// Zig Syntax Highlighting Test
/// A memory allocator and data structure library showcasing comptime and error handling.

const std = @import("std");
const Allocator = std.mem.Allocator;
const assert = std.debug.assert;
const log = std.log.scoped(.data_structures);

// Compile-time constants
const DEFAULT_CAPACITY: usize = 16;
const GROWTH_FACTOR: f64 = 1.5;
const MAX_LOAD_FACTOR: f64 = 0.75;

// Error set
const HashMapError = error{
    OutOfMemory,
    KeyNotFound,
    CapacityOverflow,
};

// Comptime generic HashMap
pub fn HashMap(comptime K: type, comptime V: type) type {
    return struct {
        const Self = @This();

        const Entry = struct {
            key: K,
            value: V,
            hash: u64,
            occupied: bool = false,
            deleted: bool = false,
        };

        entries: []Entry,
        count: usize,
        allocator: Allocator,

        pub fn init(allocator: Allocator) Self {
            return initWithCapacity(allocator, DEFAULT_CAPACITY) catch unreachable;
        }

        pub fn initWithCapacity(allocator: Allocator, capacity: usize) !Self {
            const entries = try allocator.alloc(Entry, capacity);
            @memset(entries, Entry{
                .key = undefined,
                .value = undefined,
                .hash = 0,
                .occupied = false,
                .deleted = false,
            });

            return Self{
                .entries = entries,
                .count = 0,
                .allocator = allocator,
            };
        }

        pub fn deinit(self: *Self) void {
            self.allocator.free(self.entries);
            self.* = undefined;
        }

        pub fn put(self: *Self, key: K, value: V) !void {
            if (self.shouldGrow()) {
                try self.resize(self.entries.len * 2);
            }

            const hash = computeHash(key);
            var index = @as(usize, @intCast(hash % self.entries.len));

            while (true) {
                const entry = &self.entries[index];
                if (!entry.occupied or entry.deleted) {
                    entry.* = .{
                        .key = key,
                        .value = value,
                        .hash = hash,
                        .occupied = true,
                        .deleted = false,
                    };
                    self.count += 1;
                    return;
                }
                if (entry.hash == hash and eql(entry.key, key)) {
                    entry.value = value;
                    return;
                }
                index = (index + 1) % self.entries.len;
            }
        }

        pub fn get(self: *const Self, key: K) ?V {
            const hash = computeHash(key);
            var index = @as(usize, @intCast(hash % self.entries.len));
            var probes: usize = 0;

            while (probes < self.entries.len) : (probes += 1) {
                const entry = &self.entries[index];
                if (!entry.occupied and !entry.deleted) return null;
                if (entry.occupied and entry.hash == hash and eql(entry.key, key)) {
                    return entry.value;
                }
                index = (index + 1) % self.entries.len;
            }

            return null;
        }

        pub fn remove(self: *Self, key: K) bool {
            const hash = computeHash(key);
            var index = @as(usize, @intCast(hash % self.entries.len));

            for (0..self.entries.len) |_| {
                const entry = &self.entries[index];
                if (!entry.occupied and !entry.deleted) return false;
                if (entry.occupied and entry.hash == hash and eql(entry.key, key)) {
                    entry.occupied = false;
                    entry.deleted = true;
                    self.count -= 1;
                    return true;
                }
                index = (index + 1) % self.entries.len;
            }

            return false;
        }

        pub fn iterator(self: *const Self) Iterator {
            return .{ .entries = self.entries, .index = 0 };
        }

        pub const Iterator = struct {
            entries: []const Entry,
            index: usize,

            pub fn next(it: *Iterator) ?struct { key: K, value: V } {
                while (it.index < it.entries.len) {
                    const entry = it.entries[it.index];
                    it.index += 1;
                    if (entry.occupied and !entry.deleted) {
                        return .{ .key = entry.key, .value = entry.value };
                    }
                }
                return null;
            }
        };

        fn shouldGrow(self: *const Self) bool {
            const load = @as(f64, @floatFromInt(self.count)) /
                @as(f64, @floatFromInt(self.entries.len));
            return load > MAX_LOAD_FACTOR;
        }

        fn resize(self: *Self, new_capacity: usize) !void {
            const old_entries = self.entries;
            self.entries = try self.allocator.alloc(Entry, new_capacity);
            @memset(self.entries, Entry{
                .key = undefined,
                .value = undefined,
                .hash = 0,
                .occupied = false,
                .deleted = false,
            });
            self.count = 0;

            for (old_entries) |entry| {
                if (entry.occupied and !entry.deleted) {
                    try self.put(entry.key, entry.value);
                }
            }

            self.allocator.free(old_entries);
        }

        // Comptime hash function selection
        fn computeHash(key: K) u64 {
            if (comptime K == []const u8 or K == []u8) {
                return std.hash.Wyhash.hash(0, key);
            } else if (comptime @typeInfo(K) == .int or @typeInfo(K) == .comptime_int) {
                var hasher = std.hash.Wyhash.init(0);
                hasher.update(std.mem.asBytes(&key));
                return hasher.final();
            } else {
                @compileError("Unsupported key type: " ++ @typeName(K));
            }
        }

        fn eql(a: K, b: K) bool {
            if (comptime K == []const u8) {
                return std.mem.eql(u8, a, b);
            } else {
                return a == b;
            }
        }
    };
}

// Tagged union (sum type)
const JsonValue = union(enum) {
    null_val: void,
    bool_val: bool,
    int_val: i64,
    float_val: f64,
    string_val: []const u8,
    array_val: std.ArrayList(JsonValue),
    object_val: HashMap([]const u8, JsonValue),

    pub fn format(self: JsonValue, writer: anytype) !void {
        switch (self) {
            .null_val => try writer.writeAll("null"),
            .bool_val => |b| try writer.print("{}", .{b}),
            .int_val => |i| try writer.print("{d}", .{i}),
            .float_val => |f| try writer.print("{d:.6}", .{f}),
            .string_val => |s| try writer.print("\"{s}\"", .{s}),
            .array_val => |arr| {
                try writer.writeByte('[');
                for (arr.items, 0..) |item, i| {
                    if (i > 0) try writer.writeAll(", ");
                    try item.format(writer);
                }
                try writer.writeByte(']');
            },
            .object_val => |*obj| {
                try writer.writeByte('{');
                var it = obj.iterator();
                var first = true;
                while (it.next()) |kv| {
                    if (!first) try writer.writeAll(", ");
                    try writer.print("\"{s}\": ", .{kv.key});
                    try kv.value.format(writer);
                    first = false;
                }
                try writer.writeByte('}');
            },
        }
    }
};

// Comptime string processing
fn comptimeUpperCase(comptime input: []const u8) *const [input.len]u8 {
    comptime {
        var result: [input.len]u8 = undefined;
        for (input, 0..) |c, i| {
            result[i] = if (c >= 'a' and c <= 'z') c - 32 else c;
        }
        return &result;
    }
}

// Tests
test "HashMap basic operations" {
    var map = HashMap(u32, []const u8).init(std.testing.allocator);
    defer map.deinit();

    try map.put(1, "hello");
    try map.put(2, "world");

    try std.testing.expectEqualStrings("hello", map.get(1).?);
    try std.testing.expectEqualStrings("world", map.get(2).?);
    try std.testing.expect(map.get(3) == null);

    _ = map.remove(1);
    try std.testing.expect(map.get(1) == null);
}

test "comptime upper case" {
    const result = comptimeUpperCase("hello world");
    try std.testing.expectEqualStrings("HELLO WORLD", result);
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    var map = HashMap([]const u8, i64).init(allocator);
    defer map.deinit();

    const keys = [_][]const u8{ "alpha", "beta", "gamma", "delta" };
    for (keys, 0..) |key, i| {
        try map.put(key, @as(i64, @intCast(i * 10)));
    }

    var it = map.iterator();
    while (it.next()) |kv| {
        log.info("{s} = {d}", .{ kv.key, kv.value });
    }
}
