package fixture;

import java.net.ServerSocket;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import org.apache.dubbo.common.URL;
import org.apache.dubbo.config.ApplicationConfig;
import org.apache.dubbo.config.ProtocolConfig;
import org.apache.dubbo.config.RegistryConfig;
import org.apache.dubbo.config.ServiceConfig;
import org.apache.dubbo.registry.Registry;
import org.apache.dubbo.registry.nacos.NacosRegistryFactory;
import org.apache.dubbo.rpc.model.ScopeModelUtil;

public class NacosFixture {
  private static final AtomicInteger calls = new AtomicInteger();
  private static int freePort() throws Exception {
    try (ServerSocket server = new ServerSocket(0)) { return server.getLocalPort(); }
  }
  public static void main(String[] args) throws Exception {
    String registryAddress = "nacos://127.0.0.1:18848";
    ApplicationConfig app = new ApplicationConfig("moleapi-nacos-qa");
    app.setQosEnable(false);
    app.setEnableFileCache(false);
    ServiceConfig<RegistryService> service = new ServiceConfig<>();
    service.setApplication(app);
    service.setInterface(RegistryService.class);
    service.setRef(value -> { calls.incrementAndGet(); return "nacos:" + value; });
    service.setVersion("1.0.0");
    service.setGroup("qa");
    service.setRegistry(new RegistryConfig(registryAddress));
    ProtocolConfig protocol = new ProtocolConfig("dubbo");
    protocol.setPort(freePort());
    service.setProtocol(protocol);
    Registry registry = null;
    URL query = URL.valueOf("consumer://127.0.0.1/fixture.RegistryService?category=providers&interface=fixture.RegistryService&version=1.0.0&group=qa");
    try {
      service.export();
      NacosRegistryFactory factory = new NacosRegistryFactory();
      factory.setApplicationModel(ScopeModelUtil.getApplicationModel(service.getScopeModel()));
      registry = factory.getRegistry(URL.valueOf(registryAddress + "?check=true&timeout=5000"));
      List<URL> found = List.of();
      for (int i=0;i<40;i++) {
        found = registry.lookup(query);
        if (!found.isEmpty()) break;
        Thread.sleep(250);
      }
      if (found.size()!=1) throw new IllegalStateException("Expected one provider: " + found);
      if (!registry.lookup(query.addParameter("group","wrong")).isEmpty()) throw new IllegalStateException("Group mismatch");
      if (!registry.lookup(query.addParameter("version","wrong")).isEmpty()) throw new IllegalStateException("Version mismatch");
      if (calls.get()!=0) throw new IllegalStateException("Nacos lookup invoked the provider");
      System.out.println("NACOS_PROVIDER=" + found.get(0).toFullString());
      System.out.println("NACOS_REGISTRY=" + registryAddress);
      System.out.flush();
      if (args.length==1 && args[0].equals("--hold")) new java.util.concurrent.CountDownLatch(1).await();
    } finally {
      service.unexport();
      if (registry!=null) {
        boolean removed = false;
        for (int i=0;i<40;i++) {
          if (registry.lookup(query).isEmpty()) { removed = true; break; }
          Thread.sleep(250);
        }
        registry.destroy();
        if (!removed) throw new IllegalStateException("Nacos provider remained after unexport");
      }
    }
    System.out.println("PASS NACOS lookup group version provider-removal");
    System.exit(0);
  }
}
