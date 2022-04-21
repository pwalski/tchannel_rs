package tchannel.rs.samples.server;

import com.uber.tchannel.api.ResponseCode;
import com.uber.tchannel.api.TChannel;
import com.uber.tchannel.api.handlers.RawRequestHandler;
import com.uber.tchannel.messages.RawRequest;
import com.uber.tchannel.messages.RawResponse;

import java.net.InetAddress;

public final class SyncServer {

    private SyncServer() {
    }

    public static void main(String[] args) throws Exception {
        TChannel server = createServer();
        final long start = System.currentTimeMillis();
        System.out.println(String.format("%nTime cost: %dms", System.currentTimeMillis() - start));
        server.shutdown(false);
    }

    protected static TChannel createServer() throws Exception {
        TChannel tchannel = new TChannel.Builder("server")
                .setServerHost(InetAddress.getByAddress(new byte[] { 0, 0, 0, 0 }))
                .setServerPort(8888)
                .build();
        tchannel.makeSubChannel("server")
                .register("pong", new ExampleRawHandler());
        tchannel.listen().channel().closeFuture().sync();
        return tchannel;
    }

    protected static TChannel createClient() throws Exception {
        TChannel tchannel = new TChannel.Builder("client")
                .build();
        tchannel.makeSubChannel("server");
        return tchannel;
    }

    static class ExampleRawHandler extends RawRequestHandler {
        private int count = 0;

        @Override
        public RawResponse handleImpl(RawRequest request) {
            System.out.println(String.format("Request received: header: %s, body: %s",
                    request.getHeader(),
                    request.getBody()));
            count++;
            switch (count) {
                case 1:
                    return createResponse(request, ResponseCode.OK, "Polo", "Pong!");
                case 2:
                    return createResponse(request, ResponseCode.Error, "Polo", "I feel bad ...");
                default:
                    return createResponse(request, ResponseCode.Error, "Polo", "Not again!");
            }
        }

        private RawResponse createResponse(RawRequest request, ResponseCode code, String header, String body) {
            return new RawResponse.Builder(request)
                    .setTransportHeaders(request.getTransportHeaders())
                    .setHeader(header)
                    .setBody(body)
                    .build();
        }
    }
}
