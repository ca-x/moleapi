package fixture;

public interface EchoService {
    String echo(String value);
    String join(String first, String second);
    long add(long first, long second);
    String describe(Greeting greeting);
    String names(java.util.List<String> values);
    String maybeNull(String value);
    String fail(String value);
    String attachment(String key);
}
