import org.gradle.api.tasks.bundling.Jar
import java.util.jar.JarFile

// Minimal Kotlin/JVM consumers for the UniFFI-generated Cribra bindings.
//
// The generated bindings and the native library are build outputs under the
// repository's ignored target/ tree; this project never vendors or commits them.
// One module, three source sets: `main` (Core), `flow` (Flow/Share) and `fuse`
// (Core-owned parallel scanning). Every source set compiles against exactly one
// generated binding, because all three bindings declare the same
// `uniffi.cribra_uniffi` package and must never share a classpath. Each source
// set is also packaged as its own thin profile JAR; native libraries stay
// external.

plugins {
    kotlin("jvm") version "2.4.21"
    application
}

repositories {
    mavenCentral()
}

kotlin {
    jvmToolchain(17)
}

// Overridable for a relocated checkout; defaults to the repository target/ tree.
val repositoryRoot = layout.projectDirectory.dir("../..").asFile.canonicalFile
val generatedBindings = providers.gradleProperty("cribra.kotlin.generated")
    .getOrElse(repositoryRoot.resolve("target/uniffi-kotlin-smoke/generated").absolutePath)
val nativeLibraryDir = providers.gradleProperty("cribra.kotlin.native")
    .getOrElse(repositoryRoot.resolve("target/debug").absolutePath)
val coreSmokeDir = providers.gradleProperty("cribra.kotlin.core.smoke-dir")
    .getOrElse(repositoryRoot.resolve("target/uniffi-kotlin-smoke").absolutePath)
val flowSmokeDir = providers.gradleProperty("cribra.kotlin.flow.smoke-dir")
    .getOrElse(repositoryRoot.resolve("target/uniffi-kotlin-flow-smoke").absolutePath)
val flowGeneratedBindings = File(flowSmokeDir, "generated").absolutePath
val fuseSmokeDir = providers.gradleProperty("cribra.kotlin.fuse.smoke-dir")
    .getOrElse(repositoryRoot.resolve("target/uniffi-kotlin-fuse-smoke").absolutePath)
val fuseGeneratedBindings = File(fuseSmokeDir, "generated").absolutePath
val fixtures = repositoryRoot.resolve("crates/cribra-uniffi/tests/fixtures")

sourceSets {
    main {
        kotlin.srcDir(generatedBindings)
    }
    create("flow") {
        kotlin.srcDir("src/flow/kotlin")
        kotlin.srcDir(flowGeneratedBindings)
    }
    create("fuse") {
        kotlin.srcDir("src/fuse/kotlin")
        kotlin.srcDir(fuseGeneratedBindings)
    }
}

val jna = "net.java.dev.jna:jna:5.17.0"
val jnaVersion = jna.substringAfterLast(':')
val gson = "com.google.code.gson:gson:2.13.2"

dependencies {
    implementation(jna)
    // Flow/Share and Fuse smokes read the shared JSON fixtures and Rust oracle outputs.
    add("flowImplementation", jna)
    add("flowImplementation", gson)
    add("fuseImplementation", jna)
    add("fuseImplementation", gson)
}

application {
    mainClass = "cribra.kotlin.smoke.SmokeKt"
}

tasks.named<JavaExec>("run") {
    // The generated binding uses JNA direct mapping to load `cribra_uniffi`.
    systemProperty("jna.library.path", nativeLibraryDir)
}

// Compile and run the Flow/Share smoke against the real cdylib. Fixture and
// oracle paths default to the repository layout produced by `uniffi-kotlin-flow-smoke`.
tasks.register<JavaExec>("runFlow") {
    group = "application"
    description = "Compile and run the Flow/Share consumer smoke against the real cdylib."
    dependsOn("flowClasses")
    mainClass = "cribra.kotlin.flow.FlowSmokeKt"
    classpath = sourceSets["flow"].runtimeClasspath
    systemProperty("jna.library.path", nativeLibraryDir)
    workingDir = repositoryRoot
    args(
        fixtures.resolve("flow.json").absolutePath,
        File(flowSmokeDir, "flow-oracle.json").absolutePath,
        fixtures.resolve("keyed-flow.json").absolutePath,
        fixtures.resolve("keyed-options.json").absolutePath,
        fixtures.resolve("share.json").absolutePath,
        File(flowSmokeDir, "share-oracle.json").absolutePath,
    )
}

// Compile and run the Fuse parallel-scanning smoke against the real cdylib.
tasks.register<JavaExec>("runFuse") {
    group = "application"
    description = "Compile and run the Fuse parallel-scanning smoke against the real cdylib."
    dependsOn("fuseClasses")
    mainClass = "cribra.kotlin.fuse.FuseSmokeKt"
    classpath = sourceSets["fuse"].runtimeClasspath
    systemProperty("jna.library.path", nativeLibraryDir)
    workingDir = repositoryRoot
    args(
        fixtures.resolve("parallel.json").absolutePath,
        File(fuseSmokeDir, "parallel-oracle.json").absolutePath,
    )
}

// Test-only source sets keep Gson and conformance classes out of every profile
// JAR. Each compiles the same consumers with exactly one generated binding and
// executes in a fresh JVM against that profile's native library.
for ((profile, smokeDir, generated) in listOf(
    Triple("core", coreSmokeDir, generatedBindings),
    Triple("flow", flowSmokeDir, flowGeneratedBindings),
    Triple("fuse", fuseSmokeDir, fuseGeneratedBindings),
)) {
    val conformance = sourceSets.create("${profile}Conformance") {
        kotlin.srcDir(repositoryRoot.resolve("crates/cribra-uniffi/tests/kotlin"))
        kotlin.srcDir(generated)
    }
    dependencies.add(conformance.implementationConfigurationName, jna)
    dependencies.add(conformance.implementationConfigurationName, gson)
    tasks.register<JavaExec>("run${profile.replaceFirstChar(Char::uppercase)}Conformance") {
        group = "verification"
        description = "Execute $profile configuration, streaming, incremental and generated API conformance."
        dependsOn(conformance.classesTaskName)
        mainClass = "cribra.kotlin.conformance.ConformanceSmokeKt"
        classpath = conformance.runtimeClasspath
        systemProperty("jna.library.path", providers.gradleProperty("cribra.kotlin.$profile.native").getOrElse(nativeLibraryDir))
        systemProperty("jna.nosys", "true")
        systemProperty("file.encoding", "UTF-8")
        workingDir = repositoryRoot
        args(fixtures.resolve("configurations.json").absolutePath,
            File(smokeDir, "scanner-oracle.json").absolutePath,
            fixtures.resolve("incremental.json").absolutePath,
            fixtures.resolve("api-contract.json").absolutePath, profile)
    }
}

// ---- Profile JAR packaging --------------------------------------------------
//
// Each JAR contains exactly one profile's generated UniFFI classes plus its
// profile metadata/guard class. Native libraries and third-party dependencies
// stay external: these are thin binding JARs, never fat JARs.

val cribraVersion = "0.7.2"
val adapterVersion = "0.1.0"
val uniffiVersion = "0.32.2"

data class ProfileJarSpec(
    val profile: String,
    val sourceSet: String,
    val marker: String,
    val title: String,
)

val profileSpecs = listOf(
    ProfileJarSpec("core", "main", "CoreProfile", "Cribra Kotlin/JVM Core profile"),
    ProfileJarSpec("flow", "flow", "FlowProfile", "Cribra Kotlin/JVM Flow/Share profile"),
    ProfileJarSpec("fuse", "fuse", "FuseProfile", "Cribra Kotlin/JVM Fuse profile"),
)

val profileJarTasks = profileSpecs.associate { spec ->
    spec.profile to tasks.register<Jar>("${spec.profile}Jar") {
        group = "build"
        description = "Package the ${spec.profile} profile into a standalone Kotlin/JVM JAR."
        archiveBaseName.set("cribra-kotlin-${spec.profile}")
        archiveVersion.set(cribraVersion)
        archiveClassifier.set("")
        archiveExtension.set("jar")
        destinationDirectory.set(layout.buildDirectory.dir("libs"))
        isPreserveFileTimestamps = false
        isReproducibleFileOrder = true
        from(sourceSets[spec.sourceSet].output) {
            include("uniffi/cribra_uniffi/**")
            include("cribra/kotlin/profile/${spec.marker}.class")
            // Kotlin module metadata exposes top-level generated declarations to consumers.
            include("META-INF/*.kotlin_module")
        }
        manifest {
            attributes(
                "Implementation-Title" to spec.title,
                "Implementation-Version" to cribraVersion,
                "Implementation-Vendor" to "Cribra",
                "Automatic-Module-Name" to "cribra.kotlin.${spec.profile}",
                "Cribra-Profile" to spec.profile,
                "Cribra-Core-Version" to cribraVersion,
                "Cribra-Adapter-Version" to adapterVersion,
                "Cribra-Uniffi-Version" to uniffiVersion,
                "Cribra-Native-Library" to "external",
                "Cribra-Classpath-Rule" to "exactly-one-profile",
            )
        }
    }
}

tasks.register("profileJars") {
    group = "build"
    description = "Package the Core, Flow/Share and Fuse Kotlin/JVM profile JARs."
    dependsOn(profileJarTasks.values)
}

tasks.register("verifyProfileJars") {
    group = "verification"
    description = "Verify profile JAR contents, manifests, classfile level and profile gating."
    dependsOn(profileJarTasks.values)

    doLast {
        val libs = layout.buildDirectory.dir("libs").get().asFile
        val expectedMarkers = profileSpecs.associate { it.profile to it.marker }
        val flowSurface = "uniffi/cribra_uniffi/ShareModeKind.class"

        for (spec in profileSpecs) {
            val jar = libs.resolve("cribra-kotlin-${spec.profile}-$cribraVersion.jar")
            check(jar.isFile) { "missing profile JAR: $jar" }
            JarFile(jar).use { jarFile ->
                val entries = jarFile.entries().asSequence().map { it.name }.toList()
                val classes = entries.filter { it.endsWith(".class") }

                check(classes.any { it.startsWith("uniffi/cribra_uniffi/") }) {
                    "${spec.profile}: missing generated UniFFI classes"
                }
                check("cribra/kotlin/profile/${spec.marker}.class" in entries) {
                    "${spec.profile}: missing ${spec.marker} metadata/guard"
                }
                val moduleFiles = entries.filter { it.startsWith("META-INF/") && it.endsWith(".kotlin_module") }
                check(moduleFiles.size == 1) {
                    "${spec.profile}: expected exactly one Kotlin module file, found $moduleFiles"
                }
                for ((other, marker) in expectedMarkers) {
                    if (other != spec.profile) {
                        check("cribra/kotlin/profile/$marker.class" !in entries) {
                            "${spec.profile}: contains foreign profile marker $marker"
                        }
                    }
                }
                check(
                    entries.none {
                        it.startsWith("cribra/kotlin/smoke/") ||
                            it.startsWith("cribra/kotlin/flow/") ||
                            it.startsWith("cribra/kotlin/fuse/")
                    },
                ) { "${spec.profile}: contains smoke consumer classes" }
                check(entries.none { it.endsWith(".dylib") || it.endsWith(".so") || it.endsWith(".dll") }) {
                    "${spec.profile}: embeds a native library"
                }
                check(entries.none { it.startsWith("com/google/gson/") || it.startsWith("com/sun/jna/") }) {
                    "${spec.profile}: embeds a runtime dependency"
                }

                // The Core binding must not carry Flow/Share classes; Flow and Fuse must.
                if (spec.profile == "core") {
                    check(flowSurface !in entries) { "core: contains Flow/Share classes" }
                } else {
                    check(flowSurface in entries) { "${spec.profile}: missing Flow/Share classes" }
                }

                val attributes = jarFile.getManifest().mainAttributes
                check(attributes.getValue("Cribra-Profile") == spec.profile) { "${spec.profile}: bad Cribra-Profile" }
                check(attributes.getValue("Implementation-Version") == cribraVersion) {
                    "${spec.profile}: bad Implementation-Version"
                }
                check(attributes.getValue("Cribra-Core-Version") == cribraVersion) { "${spec.profile}: bad Cribra-Core-Version" }
                check(attributes.getValue("Cribra-Adapter-Version") == adapterVersion) {
                    "${spec.profile}: bad Cribra-Adapter-Version"
                }
                check(attributes.getValue("Cribra-Uniffi-Version") == uniffiVersion) {
                    "${spec.profile}: bad Cribra-Uniffi-Version"
                }
                check(attributes.getValue("Cribra-Native-Library") == "external") {
                    "${spec.profile}: native library is not marked external"
                }
                check(attributes.getValue("Cribra-Classpath-Rule") == "exactly-one-profile") {
                    "${spec.profile}: bad Cribra-Classpath-Rule"
                }
                check(attributes.getValue("Automatic-Module-Name") == "cribra.kotlin.${spec.profile}") {
                    "${spec.profile}: bad Automatic-Module-Name"
                }

                // Every classfile must target Java 17 (major version 61).
                val classVersions = classes.map { name ->
                    val bytes = jarFile.getInputStream(jarFile.getEntry(name)).use { it.readBytes() }
                    (((bytes[6].toInt() and 0xFF) shl 8) or (bytes[7].toInt() and 0xFF))
                }.toSet()
                check(classVersions == setOf(61)) { "${spec.profile}: unexpected classfile versions $classVersions" }

                println(
                    "Verified ${jar.name}: ${classes.size} classes, ${entries.size} entries, " +
                        "classfile ${classVersions.single()}, profile '${spec.profile}'",
                )
            }
        }
    }
}

// Stage the exact third-party JAR that the native distribution bundles ship.
// Bundle assembly lives outside Gradle (bindings/kotlin/build_bundle.py); this
// task only resolves JNA from the same dependency graph the consumers use, so
// the bundled byte stream is not a hand-picked file from a global cache.
tasks.register<Copy>("stageBundleDependencies") {
    group = "build"
    description = "Stage the exact JNA JAR that the Cribra Kotlin/JVM bundles ship."
    from(configurations.runtimeClasspath) {
        include("jna-$jnaVersion.jar")
    }
    into(layout.buildDirectory.dir("bundle-deps"))
}
