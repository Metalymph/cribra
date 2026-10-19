package cribra.kotlin.verify.consumer

import com.sun.jna.NativeLibrary
import java.nio.file.Files
import java.nio.file.Path

/**
 * Asserts that JNA loaded the Cribra library from the exact file supplied by the
 * bundle qualification driver. The driver passes an absolute path outside the
 * repository and restricts JNA to the bundle's `native/` directory.
 */
internal fun assertBundleNative(expected: String) {
    check(System.getProperty("uniffi.component.cribra_uniffi.libraryOverride") == null) {
        "unexpected UniFFI library override"
    }
    val loaded = requireNotNull(NativeLibrary.getInstance("cribra_uniffi").file) {
        "JNA did not report a file for the loaded cribra_uniffi library"
    }
    val actual = loaded.toPath()
    val expectedPath = Path.of(expected)
    check(Files.isSameFile(actual, expectedPath)) {
        "loaded native library $actual, expected $expectedPath"
    }
    println("NATIVE=${actual.toRealPath()}")
}
