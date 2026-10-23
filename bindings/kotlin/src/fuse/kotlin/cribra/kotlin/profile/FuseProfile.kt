package cribra.kotlin.profile

/**
 * Metadata and classpath guard for the Cribra **Fuse** Kotlin/JVM profile JAR.
 *
 * All Cribra profile JARs expose the same `uniffi.cribra_uniffi` package, so a
 * consumer must place exactly one profile JAR on its classpath. Call
 * [requireExclusiveClasspath] once during startup to fail fast when another
 * profile JAR is also present.
 *
 * This class deliberately uses only `java.lang` APIs, so the guard works before
 * the Kotlin runtime, JNA and the generated binding are initialized.
 */
public object FuseProfile {
    /** Stable profile identifier, matching the JAR manifest's `Cribra-Profile` value. */
    public const val ID: String = "fuse"

    /** Cribra Core version this binding was generated for. */
    public const val CORE_VERSION: String = "0.7.2"

    /** Cribra UniFFI adapter version. */
    public const val ADAPTER_VERSION: String = "0.1.0"

    /** UniFFI version used to generate the binding. */
    public const val UNIFFI_VERSION: String = "0.32.2"

    /**
     * Fails fast when another Cribra profile JAR shares this classpath.
     *
     * The three profile JARs intentionally keep the same `uniffi.cribra_uniffi`
     * package, so combining them would silently hide one profile's classes
     * behind classpath order. This guard turns that situation into an immediate
     * [IllegalStateException] instead.
     */
    public fun requireExclusiveClasspath() {
        var present = false
        try {
            Class.forName("cribra.kotlin.profile.CoreProfile")
            present = true
        } catch (_: ClassNotFoundException) {
            // Expected: the Core profile is absent.
        }
        if (present) {
            throw IllegalStateException(
                "Cribra profile 'fuse' cannot share a classpath with profile 'core'; use exactly one Cribra Kotlin/JVM profile JAR",
            )
        }
        present = false
        try {
            Class.forName("cribra.kotlin.profile.FlowProfile")
            present = true
        } catch (_: ClassNotFoundException) {
            // Expected: the Flow profile is absent.
        }
        if (present) {
            throw IllegalStateException(
                "Cribra profile 'fuse' cannot share a classpath with profile 'flow'; use exactly one Cribra Kotlin/JVM profile JAR",
            )
        }
    }
}
