package io.groundinglab.pcoc;

import org.junit.jupiter.api.extension.BeforeEachCallback;
import org.junit.jupiter.api.extension.ExtensionContext;

/** Every test locates the repository before it runs. */
public final class RepoExtension implements BeforeEachCallback {
    @Override
    public void beforeEach(ExtensionContext context) {
        Repo.root();
    }
}
