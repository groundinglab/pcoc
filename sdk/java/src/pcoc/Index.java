package pcoc;

/**
 * Portable index as documents, not a network client (§14).
 * {@link #get} returns the body of {@code GET {index-url}/index/{height}}, or null
 * when that read does not return a document.
 */
public interface Index {
    byte[] get(long height);

    Index EMPTY = new Index() {
        public byte[] get(long height) {
            return null;
        }
    };
}
