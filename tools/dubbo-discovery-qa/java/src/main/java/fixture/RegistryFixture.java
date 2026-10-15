package fixture;

import java.net.ServerSocket;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import org.apache.curator.test.TestingServer;
import org.apache.dubbo.common.URL;
import org.apache.dubbo.config.ApplicationConfig;
import org.apache.dubbo.config.ProtocolConfig;
import org.apache.dubbo.config.ReferenceConfig;
import org.apache.dubbo.config.RegistryConfig;
import org.apache.dubbo.config.ServiceConfig;
import org.apache.dubbo.registry.Registry;
import org.apache.dubbo.registry.zookeeper.ZookeeperRegistryFactory;
import org.apache.dubbo.rpc.model.ScopeModelUtil;
import org.apache.dubbo.rpc.service.GenericService;

/** Real Apache registry + provider; no product source or unchecked network route. */
public class RegistryFixture {
    private static final AtomicInteger calls = new AtomicInteger();

    private static int freePort() throws Exception {
        try (ServerSocket listener = new ServerSocket(0)) { return listener.getLocalPort(); }
    }

    private static List<URL> lookup(Registry registry, URL condition, boolean present) throws Exception {
        long until = System.nanoTime() + java.util.concurrent.TimeUnit.SECONDS.toNanos(12);
        List<URL> found;
        do {
            found = registry.lookup(condition);
            if (present == !found.isEmpty()) return found;
            Thread.sleep(100);
        } while (System.nanoTime() < until);
        throw new AssertionError("Registry did not reach expected provider state: " + found);
    }

    public static void main(String[] args) throws Exception {
        try (TestingServer server = new TestingServer(true)) {
            String registryAddress = "zookeeper://" + server.getConnectString();
            ApplicationConfig app = new ApplicationConfig("moleapi-zookeeper-qa");
            app.setQosEnable(false);
            app.setEnableFileCache(false);
            ServiceConfig<RegistryService> service = new ServiceConfig<>();
            service.setApplication(app);
            service.setInterface(RegistryService.class);
            service.setRef(value -> {calls.incrementAndGet();return "echo:" + value;});
            service.setVersion("1.0.0");
            service.setGroup("qa");
            service.setRegistry(new RegistryConfig(registryAddress));
            ProtocolConfig protocol = new ProtocolConfig("dubbo");
            protocol.setPort(freePort());
            service.setProtocol(protocol);
            ServiceConfig<RegistryService> other = new ServiceConfig<>();
            other.setApplication(app);
            other.setInterface(RegistryService.class);
            other.setRef(value -> "other:" + value);
            other.setVersion("2.0.0");
            other.setGroup("other");
            other.setRegistry(new RegistryConfig(registryAddress));
            ProtocolConfig otherProtocol = new ProtocolConfig("dubbo");
            otherProtocol.setPort(freePort());
            other.setProtocol(otherProtocol);
            Registry registry = null;
            try {
                service.export();
                other.export();
                URL registryURL = URL.valueOf(registryAddress + "?check=true&timeout=5000");
                var applicationModel = ScopeModelUtil.getApplicationModel(service.getScopeModel());
                var factory = new ZookeeperRegistryFactory(applicationModel);
                factory.setApplicationModel(applicationModel);
                registry = factory.getRegistry(registryURL);
                URL query = URL.valueOf("consumer://127.0.0.1/fixture.RegistryService?category=providers&version=1.0.0&group=qa");
                List<URL> providers = lookup(registry, query, true);
                if (providers.size() != 1 || calls.get() != 0) throw new AssertionError("Lookup invoked provider or returned unexpected count");
                URL chosen = providers.get(0);
                if (!"dubbo".equals(chosen.getProtocol()) || !"fixture.RegistryService".equals(chosen.getPath())) throw new AssertionError("Wrong provider URL");
                if (!registry.lookup(query.addParameter("group", "other")).isEmpty()) throw new AssertionError("Group filter failed");
                if (!registry.lookup(query.addParameter("version", "2.0.0")).isEmpty()) throw new AssertionError("Version filter failed");
                ReferenceConfig<GenericService> reference = new ReferenceConfig<>();
                reference.setApplication(app);
                reference.setInterface("fixture.RegistryService");
                reference.setVersion("1.0.0");
                reference.setGroup("qa");
                reference.setGeneric("true");
                reference.setUrl(chosen.toFullString());
                reference.setRegistry(new RegistryConfig("N/A"));
                try {
                    if (!"echo:moleapi".equals(reference.get().$invoke("echo", new String[]{"java.lang.String"}, new Object[]{"moleapi"}))) throw new AssertionError("Selected provider did not answer direct call");
                } finally { reference.destroy(); }
                if (calls.get() != 1) throw new AssertionError("Provider invocation count changed during lookup");
                System.out.println("ZOOKEEPER_PROVIDER=" + chosen.toFullString());
                System.out.println("ZOOKEEPER_REGISTRY=" + registryAddress);
                System.out.flush();
                if (args.length == 1 && "--hold".equals(args[0])) {
                    new java.util.concurrent.CountDownLatch(1).await();
                }
                System.out.println("PASS ZOOKEEPER lookup group version direct-call provider-removal");
            } finally {
                other.unexport();
                service.unexport();
                if (registry != null) {
                    URL query = URL.valueOf("consumer://127.0.0.1/fixture.RegistryService?category=providers&version=1.0.0&group=qa");
                    lookup(registry, query, false);
                    registry.destroy();
                }
            }
        }
        System.exit(0);
    }
}
