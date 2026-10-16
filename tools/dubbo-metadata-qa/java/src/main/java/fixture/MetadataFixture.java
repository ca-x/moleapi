package fixture;
import java.net.ServerSocket;
import java.util.concurrent.atomic.AtomicInteger;
import org.apache.curator.test.TestingServer;
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

public class MetadataFixture {
  static final AtomicInteger calls = new AtomicInteger();
  static int freePort() throws Exception { try(ServerSocket server=new ServerSocket(0)){return server.getLocalPort();} }
  public static void main(String[] args) throws Exception {
    try (TestingServer zk = new TestingServer(true)) {
      boolean secure=args.length==1 && args[0].startsWith("--auth");
      String endpoint="zookeeper://"+zk.getConnectString();
      String address=secure ? "zookeeper://fixture-reader:fixture-password@"+zk.getConnectString() : endpoint;
      ApplicationConfig app=new ApplicationConfig("moleapi-metadata-qa");
      app.setQosEnable(false); app.setEnableFileCache(false);
      ServiceConfig<RegistryService> service=new ServiceConfig<>();
      service.setInterface(RegistryService.class); service.setRef(new RegistryService() {
        public String echo(String value) {calls.incrementAndGet(); return "metadata:"+value;}
        public Profile exchange(Profile value) {calls.incrementAndGet(); return value;}
      });
      service.setVersion("1.0.0"); service.setGroup("qa");
      ProtocolConfig protocol=new ProtocolConfig("dubbo"); protocol.setPort(freePort());
      DubboBootstrap bootstrap=DubboBootstrap.getInstance();
      try {
        bootstrap.application(app).registry(new RegistryConfig(address))
          .metadataReport(new MetadataReportConfig(address)).protocol(protocol).service(service).start();
        var model=ScopeModelUtil.getApplicationModel(service.getScopeModel());
        var report=model.getBeanFactory().getBean(MetadataReportInstance.class)
          .getMetadataReports(false).values().iterator().next();
        var id=new MetadataIdentifier("fixture.RegistryService","1.0.0","qa","provider","moleapi-metadata-qa");
        String definition=report.getServiceDefinition(id);
        if(definition==null || !definition.contains("echo")) throw new IllegalStateException("Missing service definition: "+definition);
        if(calls.get()!=0) throw new IllegalStateException("Metadata lookup invoked provider");
        System.out.println("METADATA_REGISTRY="+endpoint);
        System.out.println("METADATA_PROVIDER_PORT="+protocol.getPort());
        System.out.println("METADATA_PATH=/dubbo/"+id.getUniqueKey(KeyTypeEnum.PATH));
        System.out.println("METADATA_JSON="+definition);
        System.out.flush();
        if(args.length==1 && args[0].endsWith("hold")) new java.util.concurrent.CountDownLatch(1).await();
      } finally {bootstrap.stop();}
    }
    System.out.println("PASS APACHE metadata report provider-definition no-invocation");
    System.exit(0);
  }
}
