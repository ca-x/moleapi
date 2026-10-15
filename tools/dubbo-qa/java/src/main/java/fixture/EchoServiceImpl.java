package fixture;

public class EchoServiceImpl implements EchoService {
    public String echo(String value) { return "echo:" + value; }
    public String join(String first, String second) { return first + "|" + second; }
    public long add(long first, long second) { return Math.addExact(first, second); }
    public String describe(Greeting greeting) { return greeting.getName() + ":" + greeting.getCount(); }
    public String names(java.util.List<String> values) { return String.join("|", values); }
    public String maybeNull(String value) { return "null".equals(value) ? null : value; }
    public String fail(String value) { throw new IllegalArgumentException("fixture:" + value); }
    public String attachment(String key) { return org.apache.dubbo.rpc.RpcContext.getServerAttachment().getAttachment(key); }
}
