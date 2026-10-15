package fixture;

import org.apache.dubbo.config.ApplicationConfig;
import org.apache.dubbo.config.ProtocolConfig;
import org.apache.dubbo.config.RegistryConfig;
import org.apache.dubbo.config.ServiceConfig;

public class Provider {
    public static void main(String[] args) throws Exception {
        int port = Integer.parseInt(args[0]);
        ApplicationConfig application = new ApplicationConfig("moleapi-dubbo-qa");
        ServiceConfig<EchoService> service = new ServiceConfig<>();
        service.setApplication(application);
        service.setRegistry(new RegistryConfig("N/A"));
        ProtocolConfig protocol = new ProtocolConfig("dubbo");
        protocol.setPort(port);
        service.setProtocol(protocol);
        service.setInterface(EchoService.class);
        service.setRef(new EchoServiceImpl());
        service.setVersion("1.0.0");
        service.export();
        ServiceConfig<EchoService> grouped = new ServiceConfig<>();
        grouped.setApplication(application);
        grouped.setRegistry(new RegistryConfig("N/A"));
        grouped.setProtocol(protocol);
        grouped.setInterface(EchoService.class);
        grouped.setRef(new EchoServiceImpl());
        grouped.setVersion("1.0.0");
        grouped.setGroup("qa");
        grouped.export();
        System.out.println("READY " + port);
        System.out.flush();
        new java.util.concurrent.CountDownLatch(1).await();
    }
}
