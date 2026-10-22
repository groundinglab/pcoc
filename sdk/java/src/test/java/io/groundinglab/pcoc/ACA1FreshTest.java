package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

import org.junit.jupiter.api.Test;

/** AC-A1. Every file under sdk/java is new since the branch commit that removed the old tree. */
public class ACA1FreshTest {
    @Test
    public void addedOnThisBranch() throws Exception {
        Path javaDir = Path.of("").toAbsolutePath();
        Path repo = javaDir.getParent().getParent();
        assertTrue(Files.isDirectory(javaDir.resolve("src/main/java")), "sdk/java sources");
        String listed = git(repo, "ls-tree", "-r", "--name-only", "a786e6e", "--", "sdk/java");
        assertEquals("", listed.trim(), "sdk/java is empty at the branch base");
        assertTrue(Files.list(javaDir.resolve("src/main/java/io/groundinglab/pcoc")).findAny().isPresent());
    }

    static String git(Path repo, String... args) throws Exception {
        List<String> cmd = new ArrayList<String>();
        cmd.add("git");
        for (String a : args) {
            cmd.add(a);
        }
        Process p = new ProcessBuilder(cmd).directory(repo.toFile()).redirectErrorStream(true).start();
        byte[] out = p.getInputStream().readAllBytes();
        int code = p.waitFor();
        assertEquals(0, code, new String(out));
        return new String(out);
    }
}
