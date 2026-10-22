package io.groundinglab.pcoc;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * CASES synthetic index served at {@code http://127.0.0.1/index/{height}}.
 * {@link #get} is that GET. A height the fixture does not define is a 404,
 * which this index reports as no document.
 */
public final class LoopbackIndex implements Index, AutoCloseable {
    private final ServerSocket socket;
    private final Thread thread;
    private final SyntheticIndex synthetic = new SyntheticIndex();
    private final HttpClient client;
    private final String base;
    private final AtomicInteger gets = new AtomicInteger();
    private volatile boolean open = true;

    public LoopbackIndex() throws IOException {
        socket = new ServerSocket();
        socket.setReuseAddress(true);
        socket.bind(new InetSocketAddress(InetAddress.getByName("127.0.0.1"), 0));
        if (!socket.getInetAddress().isLoopbackAddress()) {
            socket.close();
            throw new IOException("index bound off loopback");
        }
        base = "http://127.0.0.1:" + socket.getLocalPort();
        client = HttpClient.newBuilder()
                .version(HttpClient.Version.HTTP_1_1)
                .connectTimeout(Duration.ofSeconds(2))
                .build();
        thread = new Thread(this::serve, "pcoc-index");
        thread.setDaemon(true);
        thread.start();
    }

    public String base() {
        return base;
    }

    public int gets() {
        return gets.get();
    }

    public byte[] get(long height) {
        if (height < 0) {
            return null;
        }
        HttpRequest req = HttpRequest.newBuilder(URI.create(base + "/index/" + Long.toString(height)))
                .timeout(Duration.ofSeconds(2))
                .version(HttpClient.Version.HTTP_1_1)
                .GET()
                .build();
        try {
            HttpResponse<byte[]> resp = client.send(req, HttpResponse.BodyHandlers.ofByteArray());
            if (resp.statusCode() != 200) {
                return null;
            }
            return resp.body();
        } catch (IOException e) {
            return null;
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return null;
        }
    }

    public void close() throws IOException {
        open = false;
        socket.close();
        thread.interrupt();
    }

    private void serve() {
        while (open) {
            try (Socket conn = socket.accept()) {
                conn.setSoTimeout(2000);
                handle(conn);
            } catch (IOException e) {
                if (!open) {
                    return;
                }
            }
        }
    }

    private void handle(Socket conn) throws IOException {
        byte[] head = readHeaders(conn.getInputStream());
        String text = new String(head, StandardCharsets.US_ASCII);
        int lineEnd = text.indexOf("\r\n");
        String line = lineEnd < 0 ? text : text.substring(0, lineEnd);
        OutputStream out = conn.getOutputStream();
        if (!line.startsWith("GET /index/")) {
            write(out, 404, new byte[0]);
            return;
        }
        String rest = line.substring("GET /index/".length());
        int sp = rest.indexOf(' ');
        String token = sp < 0 ? rest : rest.substring(0, sp);
        int q = token.indexOf('?');
        if (q >= 0) {
            token = token.substring(0, q);
        }
        gets.incrementAndGet();
        if (!digits(token)) {
            write(out, 404, new byte[0]);
            return;
        }
        long height;
        try {
            height = Long.parseLong(token);
        } catch (NumberFormatException e) {
            write(out, 404, new byte[0]);
            return;
        }
        byte[] body = synthetic.get(height);
        if (body == null) {
            write(out, 404, new byte[0]);
            return;
        }
        write(out, 200, body);
    }

    private static boolean digits(String token) {
        if (token.isEmpty()) {
            return false;
        }
        for (int i = 0; i < token.length(); i++) {
            char c = token.charAt(i);
            if (c < '0' || c > '9') {
                return false;
            }
        }
        return true;
    }

    private static byte[] readHeaders(InputStream in) throws IOException {
        ByteArrayOutputStream buf = new ByteArrayOutputStream();
        while (true) {
            int b = in.read();
            if (b < 0) {
                throw new IOException("eof");
            }
            buf.write(b);
            int n = buf.size();
            if (n > 8192) {
                throw new IOException("headers");
            }
            if (n >= 4) {
                byte[] a = buf.toByteArray();
                if (a[n - 4] == '\r' && a[n - 3] == '\n' && a[n - 2] == '\r' && a[n - 1] == '\n') {
                    return a;
                }
            }
        }
    }

    private static void write(OutputStream out, int status, byte[] body) throws IOException {
        String reason = status == 200 ? "OK" : "Not Found";
        byte[] head = ("HTTP/1.1 " + status + " " + reason + "\r\n"
                + "Content-Length: " + body.length + "\r\n"
                + "Connection: close\r\n"
                + "\r\n").getBytes(StandardCharsets.US_ASCII);
        out.write(head);
        out.write(body);
        out.flush();
    }
}
