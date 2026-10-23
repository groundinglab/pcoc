plugins {
    kotlin("jvm") version "2.0.21"
    application
}

group = "io.groundinglab"
version = "2.2.0"

repositories {
    mavenCentral()
}

kotlin {
    jvmToolchain(21)
}

val jvmVectorArgs = listOf(
    "--add-modules=jdk.incubator.vector",
    "-XX:+UseSHA",
    "-XX:+UnlockDiagnosticVMOptions",
    "-XX:+UseSHA256Intrinsics"
)

tasks.withType<JavaCompile>().configureEach {
    options.compilerArgs.addAll(listOf("--add-modules", "jdk.incubator.vector"))
}

tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile>().configureEach {
    compilerOptions {
        freeCompilerArgs.addAll(
            listOf(
                "-Xjdk-release=21",
                "-Xjvm-default=all"
            )
        )
    }
}

dependencies {
    testImplementation(kotlin("test"))
    testImplementation(project(":negatives"))
}

application {
    mainClass.set("satoshihike.CliKt")
    applicationDefaultJvmArgs = jvmVectorArgs
}

tasks.test {
    useJUnitPlatform()
    jvmArgs(jvmVectorArgs)
    maxHeapSize = "2g"
    workingDir = project.projectDir.parentFile
}

tasks.register<Jar>("fatJar") {
    group = "build"
    description = "Builds a self-contained executable JAR for the verify and producer CLI"
    archiveFileName.set("satoshi-hike-all.jar")
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
    manifest {
        attributes["Main-Class"] = "satoshihike.CliKt"
    }
    from(sourceSets["main"].output)
    dependsOn(configurations.runtimeClasspath)
    from({
        configurations.runtimeClasspath.get()
            .filter { it.name.endsWith("jar") }
            .map { zipTree(it) }
    })
}

tasks.register<JavaExec>("genObjects") {
    group = "satoshi-hike"
    description = "Generate offline test-plane objects O1-S1.cbor, O2-S2.cbor, and O3-PROD.cbor"
    classpath = sourceSets["main"].runtimeClasspath
    mainClass.set("satoshihike.CliKt")
    jvmArgs(jvmVectorArgs)
    maxHeapSize = "2g"
    workingDir = project.projectDir.parentFile
    args("gen-objects", "objects")
}

tasks.register<JavaExec>("benchmark") {
    group = "satoshi-hike"
    description = "Benchmark scalar vs optimised terrain fill and production leg"
    classpath = sourceSets["main"].runtimeClasspath
    mainClass.set("satoshihike.CliKt")
    jvmArgs(jvmVectorArgs)
    maxHeapSize = "2g"
    workingDir = project.projectDir.parentFile
    args("benchmark")
}
