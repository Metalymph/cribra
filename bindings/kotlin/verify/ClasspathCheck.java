import java.lang.reflect.InvocationTargetException;

/**
 * Verifies the profile JAR "exactly one profile per classpath" rule.
 *
 * It uses only reflection and java.lang, so it runs before the Kotlin runtime,
 * JNA or the generated binding are initialized. Run it with JDK 17 source-file
 * mode:
 *
 *   java --class-path cribra-kotlin-core-0.7.1.jar verify/ClasspathCheck.java core single
 *   java --class-path core.jar:flow.jar verify/ClasspathCheck.java core mixed
 */
public final class ClasspathCheck {
    private ClasspathCheck() {
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("usage: ClasspathCheck <core|flow|fuse> <single|mixed>");
        }
        String profile = args[0];
        String mode = args[1];
        String marker = switch (profile) {
            case "core" -> "cribra.kotlin.profile.CoreProfile";
            case "flow" -> "cribra.kotlin.profile.FlowProfile";
            case "fuse" -> "cribra.kotlin.profile.FuseProfile";
            default -> throw new IllegalArgumentException("unknown profile: " + profile);
        };

        Class<?> type = Class.forName(marker);
        Object instance = type.getField("INSTANCE").get(null);
        try {
            type.getMethod("requireExclusiveClasspath").invoke(instance);
        } catch (InvocationTargetException error) {
            Throwable cause = error.getCause();
            if (!"mixed".equals(mode)) {
                throw new AssertionError(profile + " unexpectedly rejected a single-profile classpath", cause);
            }
            if (!(cause instanceof IllegalStateException)) {
                throw new AssertionError(profile + " rejected a mixed classpath with the wrong failure type", cause);
            }
            System.out.println("PASS: " + profile + " rejected a mixed classpath: " + cause.getMessage());
            return;
        }
        if (!"single".equals(mode)) {
            throw new AssertionError(profile + " accepted a mixed classpath");
        }
        System.out.println("PASS: " + profile + " accepted a single-profile classpath");
    }
}
