plugins {
    kotlin("jvm")
    application
}

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

dependencies {
    implementation(rootProject)
}

application {
    mainClass.set("negatives.CandidateGeneratorKt")
    applicationDefaultJvmArgs = jvmVectorArgs
}

tasks.register<JavaExec>("genCandidates") {
    group = "satoshi-hike"
    description = "Deterministically generate all candidate files in candidates/"
    classpath = sourceSets["main"].runtimeClasspath
    mainClass.set("negatives.CandidateGeneratorKt")
    jvmArgs(jvmVectorArgs)
    maxHeapSize = "2g"
    workingDir = rootProject.projectDir.parentFile
    args("candidates")
}
