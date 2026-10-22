package io.groundinglab.pcoc;

import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Locates this checkout from the working directory. A test fails, and does not
 * skip, when no parent holds both {@code sdk/VERSION} and {@code spec/}.
 */
public final class Repo {
    private Repo() {}

    public static Path root() {
        Path dir = Path.of("").toAbsolutePath().normalize();
        while (dir != null) {
            if (Files.isRegularFile(dir.resolve("sdk").resolve("VERSION"))
                    && Files.isDirectory(dir.resolve("spec"))) {
                return dir;
            }
            Path parent = dir.getParent();
            if (parent == null) {
                break;
            }
            dir = parent;
        }
        throw new AssertionError("cannot locate the repository: no directory holding sdk/VERSION and spec/ "
                + "was found by walking up from the working directory");
    }

    public static Path javaDir() {
        return root().resolve("sdk").resolve("java");
    }
}
