T=~/interop-r2/toolchains
export PATH=$T/zig:$T/jdk21/bin:$T/node22/bin:$T/swift/usr/bin:$PATH
export JAVA_HOME=$T/jdk21
export GRADLE_USER_HOME=~/interop-r2/gradle-home
export npm_config_cache=~/interop-r2/npm-cache
export ZIG_GLOBAL_CACHE_DIR=~/interop-r2/zig-cache
[ -x $T/dotnet8/dotnet ] && export DOTNET_ROOT=$T/dotnet8 PATH=$T/dotnet8:$PATH
export DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1 NUGET_PACKAGES=~/interop-r2/nuget
