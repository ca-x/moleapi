package fixture;

import java.util.LinkedHashMap;
import java.util.Map;
import org.apache.dubbo.config.ApplicationConfig;
import org.apache.dubbo.config.ReferenceConfig;
import org.apache.dubbo.rpc.service.GenericService;

public class GenericConsumer {
    public static void main(String[] args) {
        int port = Integer.parseInt(args[0]);
        ReferenceConfig<GenericService> reference = new ReferenceConfig<>();
        reference.setApplication(new ApplicationConfig("moleapi-dubbo-qa-generic"));
        reference.setInterface("fixture.EchoService");
        reference.setVersion("1.0.0");
        reference.setGeneric("true");
        reference.setUrl("dubbo://127.0.0.1:" + port + "/fixture.EchoService");
        GenericService client = reference.get();
        if (!"echo:moleapi".equals(client.$invoke("echo", new String[]{"java.lang.String"}, new Object[]{"moleapi"}))) throw new AssertionError("generic echo");
        if (!"first|second".equals(client.$invoke("join", new String[]{"java.lang.String", "java.lang.String"}, new Object[]{"first", "second"}))) throw new AssertionError("generic join");
        Object sum = client.$invoke("add", new String[]{"long", "long"}, new Object[]{9007199254740993L, 2L});
        if (!(sum instanceof Number) || ((Number)sum).longValue() != 9007199254740995L) throw new AssertionError("generic long");
        Map<String, Object> greeting = new LinkedHashMap<>();
        greeting.put("class", "fixture.Greeting");
        greeting.put("name", "鼹鼠");
        greeting.put("count", 0);
        if (!"鼹鼠:0".equals(client.$invoke("describe", new String[]{"fixture.Greeting"}, new Object[]{greeting}))) throw new AssertionError("generic DTO");
        if (client.$invoke("maybeNull", new String[]{"java.lang.String"}, new Object[]{"null"}) != null) throw new AssertionError("generic null");
        try {
            client.$invoke("fail", new String[]{"java.lang.String"}, new Object[]{"expected"});
            throw new AssertionError("generic exception missing");
        } catch (RuntimeException expected) {
            if (!expected.toString().contains("fixture:expected")) throw expected;
        }
        System.out.println("GENERIC_CLIENT_PASS echo join long DTO null exception");
        reference.destroy();
    }
}
