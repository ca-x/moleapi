package fixture;

import org.apache.dubbo.config.ApplicationConfig;
import org.apache.dubbo.config.ReferenceConfig;

public class Consumer {
    public static void main(String[] args) {
        int port = Integer.parseInt(args[0]);
        ReferenceConfig<EchoService> reference = new ReferenceConfig<>();
        reference.setApplication(new ApplicationConfig("moleapi-dubbo-qa-consumer"));
        reference.setInterface(EchoService.class);
        reference.setVersion("1.0.0");
        reference.setUrl("dubbo://127.0.0.1:" + port + "/fixture.EchoService");
        EchoService client = reference.get();
        if (!"echo:moleapi".equals(client.echo("moleapi"))) throw new AssertionError("echo");
        if (!"first|second".equals(client.join("first", "second"))) throw new AssertionError("join");
        if (client.add(9007199254740993L, 2L) != 9007199254740995L) throw new AssertionError("long");
        Greeting greeting = new Greeting();
        greeting.setName("鼹鼠");
        greeting.setCount(0);
        if (!"鼹鼠:0".equals(client.describe(greeting))) throw new AssertionError("DTO");
        if (client.maybeNull("null") != null) throw new AssertionError("null");
        try {
            client.fail("expected");
            throw new AssertionError("exception not returned");
        } catch (IllegalArgumentException expected) {
            if (!expected.getMessage().contains("fixture:expected")) throw expected;
        }
        System.out.println("OFFICIAL_CLIENT_PASS echo join long DTO null exception");
        reference.destroy();
    }
}
