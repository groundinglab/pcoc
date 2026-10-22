plugins {
    java
    application
}

group = "io.groundinglab.pcoc"
version = "2.2.0"

repositories {
    mavenCentral()
}

java {
    sourceCompatibility = JavaVersion.VERSION_21
    targetCompatibility = JavaVersion.VERSION_21
}

application {
    mainClass.set("io.groundinglab.pcoc.cli.Main")
    applicationDefaultJvmArgs = listOf("-XX:+UnlockDiagnosticVMOptions", "-Xmx2g")
}

val gen = sourceSets.create("gen")

dependencies {
    testImplementation("org.junit.jupiter:junit-jupiter:5.11.4")
    "genImplementation"(sourceSets.main.get().output)
}

sourceSets.named("test") {
    compileClasspath += gen.output
    runtimeClasspath += gen.output
}

tasks.withType<JavaCompile>().configureEach {
    options.release.set(21)
    options.encoding = "UTF-8"
}

tasks.named("compileTestJava") {
    dependsOn("compileGenJava")
}

tasks.jar {
    archiveBaseName.set("pcoc")
    manifest {
        attributes("Main-Class" to "io.groundinglab.pcoc.cli.Main")
    }
}

tasks.register<Jar>("genJar") {
    group = "build"
    description = "Generator jar. Not a release artefact."
    archiveBaseName.set("pcoc-gen")
    from(gen.output)
    from(sourceSets.main.get().output)
    dependsOn(tasks.named("compileGenJava"), tasks.named("classes"))
    manifest {
        attributes("Main-Class" to "io.groundinglab.pcoc.gen.GenCandidates")
    }
}

tasks.named<JavaExec>("run") {
    jvmArgs("-XX:+UnlockDiagnosticVMOptions", "-Xmx2g")
}

tasks.test {
    useJUnitPlatform()
    maxParallelForks = 1
    dependsOn(tasks.jar, tasks.named("genJar"))
    jvmArgs("-XX:+UnlockDiagnosticVMOptions", "-Xmx2g")
    testLogging {
        events("passed", "failed", "skipped")
        showStandardStreams = false
        exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
    }
}
