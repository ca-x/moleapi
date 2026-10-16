package fixture;

import java.net.ServerSocket;
import java.util.concurrent.atomic.AtomicInteger;
import org.apache.dubbo.config.ApplicationConfig;
import org.apache.dubbo.config.MetadataReportConfig;
import org.apache.dubbo.config.ProtocolConfig;
import org.apache.dubbo.config.RegistryConfig;
import org.apache.dubbo.config.ServiceConfig;
import org.apache.dubbo.config.bootstrap.DubboBootstrap;
import org.apache.dubbo.metadata.report.MetadataReportInstance;
import org.apache.dubbo.metadata.report.identifier.MetadataIdentifier;
import org.apache.dubbo.metadata.report.identifier.KeyTypeEnum;
import org.apache.dubbo.rpc.model.ScopeModelUtil;

public class NacosMetadataFixture {
  static final AtomicInteger calls = new AtomicInteger();
  static int freePort() throws Exception { try (ServerSocket socket = new ServerSocket(0)) { return socket.getLocalPort(); } }
  public static void main(String[] args) throws Exception {
    boolean secured = args.length == 1 && args[0].equals("--auth-namespace-hold");
    String endpoint = "nacos://127.0.0.1:18848";
    String registryAddress = secured
      ? "nacos://nacos:fixture-password@127.0.0.1:18848?namespace=moleapi-qa" : endpoint;
    String metadataAddress = secured
      ? endpoint + "?username=nacos&password=fixture-password&namespace=moleapi-qa&group=moleapi-metadata-qa"
      : endpoint;
    ApplicationConfig app = new ApplicationConfig("moleapi-nacos-metadata-qa");
    app.setQosEnable(false); app.setEnableFileCache(false);
    ServiceConfig<MetadataService> service = new ServiceConfig<>();
    service.setInterface(MetadataService.class);
    service.setRef(value -> { calls.incrementAndGet(); return value; });
    service.setVersion("1.0.0"); service.setGroup("qa");
    ProtocolConfig protocol = new ProtocolConfig("dubbo"); protocol.setPort(freePort());
    DubboBootstrap bootstrap = DubboBootstrap.getInstance();
    try {
      bootstrap.application(app).registry(new RegistryConfig(registryAddress))
        .metadataReport(new MetadataReportConfig(metadataAddress)).protocol(protocol).service(service).start();
      var model = ScopeModelUtil.getApplicationModel(service.getScopeModel());
      var report = model.getBeanFactory().getBean(MetadataReportInstance.class)
        .getMetadataReports(false).values().iterator().next();
      var id = new MetadataIdentifier("fixture.MetadataService", "1.0.0", "qa", "provider", "moleapi-nacos-metadata-qa");
      String definition = null;
      for (int attempt = 0; attempt < 40; attempt++) {
        definition = report.getServiceDefinition(id);
        if (definition != null && definition.contains("exchange")) break;
        Thread.sleep(250);
      }
      if (definition == null || !definition.contains("exchange")) throw new IllegalStateException("Missing Nacos metadata definition");
      if (calls.get() != 0) throw new IllegalStateException("Metadata lookup invoked provider");
      System.out.println("METADATA_REGISTRY=" + endpoint);
      System.out.println("METADATA_DATA_ID=" + id.getUniqueKey(KeyTypeEnum.UNIQUE_KEY));
      System.out.println("METADATA_NAMESPACE=" + (secured ? "moleapi-qa" : ""));
      System.out.println("METADATA_GROUP=" + (secured ? "moleapi-metadata-qa" : "dubbo"));
      System.out.println("METADATA_JSON=" + definition);
      System.out.flush();
      if (args.length == 1 && args[0].endsWith("hold")) new java.util.concurrent.CountDownLatch(1).await();
    } finally { bootstrap.stop(); }
    System.out.println("PASS NACOS metadata report provider-definition no-invocation");
    System.exit(0);
  }
}
