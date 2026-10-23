const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{
        .preferred_optimize_mode = .ReleaseFast,
    });

    const mod = b.addModule("hike", .{
        .root_source_file = b.path("src/root.zig"),
        .target = target,
        .optimize = optimize,
    });

    const exe = b.addExecutable(.{
        .name = "hike",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = optimize,
            .imports = &.{.{ .name = "hike", .module = mod }},
        }),
    });
    b.installArtifact(exe);

    const gen = b.addExecutable(.{
        .name = "gen-candidates",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/gen_main.zig"),
            .target = target,
            .optimize = optimize,
            .imports = &.{.{ .name = "hike", .module = mod }},
        }),
    });
    b.installArtifact(gen);

    const run_step = b.step("run", "Run hike");
    const run_cmd = b.addRunArtifact(exe);
    run_step.dependOn(&run_cmd.step);
    run_cmd.step.dependOn(b.getInstallStep());
    if (b.args) |args| run_cmd.addArgs(args);

    const gen_step = b.step("candidates", "Write candidates/");
    const gen_cmd = b.addRunArtifact(gen);
    gen_step.dependOn(&gen_cmd.step);
    gen_cmd.step.dependOn(b.getInstallStep());
    if (b.args) |args| gen_cmd.addArgs(args);

    const mod_tests = b.addTest(.{ .root_module = mod });
    const run_mod_tests = b.addRunArtifact(mod_tests);
    const gen_tests = b.addTest(.{ .root_module = gen.root_module });
    const run_gen_tests = b.addRunArtifact(gen_tests);

    const test_step = b.step("test", "Unit tests and candidate idempotence");
    test_step.dependOn(&run_mod_tests.step);
    test_step.dependOn(&run_gen_tests.step);
}
