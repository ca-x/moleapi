package app.moleapi.dubbo;

import com.alibaba.fastjson2.JSON;
import com.alibaba.fastjson2.JSONArray;
import com.alibaba.fastjson2.JSONObject;
import com.alibaba.fastjson2.JSONWriter;
import com.google.common.net.InetAddresses;
import java.math.BigInteger;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.apache.dubbo.config.ApplicationConfig;
import org.apache.dubbo.config.ReferenceConfig;
import org.apache.dubbo.config.RegistryConfig;
import org.apache.dubbo.rpc.RpcContext;
import org.apache.dubbo.rpc.service.GenericException;
import org.apache.dubbo.rpc.service.GenericService;

/** One bounded classic Dubbo call per process; the Rust host owns admission and cancellation. */
public final class Worker {
    private static final int MAX_INPUT = 1024 * 1024;
    private static final int MAX_OUTPUT = 1024 * 1024;
    private static final Set<String> INPUT_KEYS = Set.of("format", "endpoint", "service", "method", "version", "group", "timeout_ms", "parameter_types", "argument_sources", "attachments");

    private Worker() {}

    public static void main(String[] ignored) throws Exception {
        try {
            byte[] bytes = System.in.readNBytes(MAX_INPUT + 1);
            if (bytes.length > MAX_INPUT) throw new IllegalArgumentException("Dubbo input exceeds 1 MiB");
            JSONObject input = JSON.parseObject(new String(bytes, StandardCharsets.UTF_8));
            if (input == null || !INPUT_KEYS.containsAll(input.keySet()) || !"moleapi-dubbo-call-v1".equals(input.getString("format"))) throw new IllegalArgumentException("Invalid Dubbo worker input");
            write(call(input));
        } catch (GenericException remote) {
            write(error("remote_exception", remote.getExceptionClass(), remote.getExceptionMessage()));
        } catch (Exception failure) {
            write(error("worker_error", failure.getClass().getName(), failure.getMessage()));
        }
    }

    private static JSONObject call(JSONObject input) throws Exception {
        String service = required(input.getString("service"), 256);
        String method = required(input.getString("method"), 128);
        if (!service.matches("[A-Za-z_$][A-Za-z0-9_$]*(\\.[A-Za-z_$][A-Za-z0-9_$]*)+") || !method.matches("[A-Za-z_$][A-Za-z0-9_$]*")) throw new IllegalArgumentException("Invalid service or method");
        URI endpoint = URI.create(required(input.getString("endpoint"), 512));
        String host = endpoint.getHost();
        String literal = host != null && host.startsWith("[") && host.endsWith("]") ? host.substring(1, host.length() - 1) : host;
        if (!"dubbo".equals(endpoint.getScheme()) || literal == null || !InetAddresses.isInetAddress(literal) || endpoint.getPort() < 1 || endpoint.getPort() > 65535 || endpoint.getRawUserInfo() != null || endpoint.getRawQuery() != null || endpoint.getRawFragment() != null || !('/' + service).equals(endpoint.getRawPath())) throw new IllegalArgumentException("Dubbo endpoint requires a checked literal IP and selected service path");
        int timeout = input.getIntValue("timeout_ms");
        if (timeout < 1 || timeout > 120_000) throw new IllegalArgumentException("Invalid Dubbo timeout");
        JSONArray names = input.getJSONArray("parameter_types");
        JSONArray sources = input.getJSONArray("argument_sources");
        if (names == null || sources == null || names.size() != sources.size() || names.size() > 32) throw new IllegalArgumentException("Dubbo arguments/types mismatch");
        String[] types = new String[names.size()];
        Object[] values = new Object[names.size()];
        for (int index = 0; index < names.size(); index++) {
            types[index] = required(names.getString(index), 256);
            if (!types[index].matches("[A-Za-z_$][A-Za-z0-9_$.]*(\\[\\])*")) throw new IllegalArgumentException("Invalid Dubbo parameter type");
            String source = required(sources.getString(index), 64 * 1024);
            values[index] = argument(types[index], source);
        }
        JSONObject attachments = input.getJSONObject("attachments");
        if (attachments != null && attachments.size() > 64) throw new IllegalArgumentException("Too many Dubbo attachments");
        ReferenceConfig<GenericService> reference = new ReferenceConfig<>();
        ApplicationConfig application = new ApplicationConfig("moleapi-dubbo-worker");
        application.setQosEnable(false);
        application.setEnableFileCache(false);
        reference.setApplication(application);
        reference.setRegistry(new RegistryConfig("N/A"));
        reference.setInterface(service);
        reference.setGeneric("true");
        reference.setUrl(endpoint.toString());
        reference.setTimeout(timeout);
        reference.setRetries(0);
        String version = input.getString("version");
        String group = input.getString("group");
        if (version != null && !version.isEmpty()) reference.setVersion(required(version, 128));
        if (group != null && !group.isEmpty()) reference.setGroup(required(group, 128));
        try {
            GenericService client = reference.get();
            if (attachments != null) {
                for (Map.Entry<String, Object> entry : attachments.entrySet()) {
                    String key = required(entry.getKey(), 128);
                    if (!(entry.getValue() instanceof String value)) throw new IllegalArgumentException("Dubbo attachment value must be text");
                    required(value, 4096);
                    if (!key.matches("[A-Za-z0-9_.-]+") || Set.of("path", "version", "group", "interface", "method").contains(key.toLowerCase())) throw new IllegalArgumentException("Invalid Dubbo attachment");
                    RpcContext.getClientAttachment().setAttachment(key, value);
                }
            }
            Object value = client.$invoke(method, types, values);
            JSONObject output = new JSONObject();
            output.put("ok", true);
            output.put("value", value);
            return output;
        } finally {
            reference.destroy();
        }
    }

    private static Object argument(String type, String source) {
        Object value = JSON.parse(source);
        if (value == null) return null;
        if ("long".equals(type) || "java.lang.Long".equals(type)) return new BigInteger(value.toString()).longValueExact();
        if ("int".equals(type) || "java.lang.Integer".equals(type)) return new BigInteger(value.toString()).intValueExact();
        if ("java.lang.String".equals(type)) {
            if (!(value instanceof String)) throw new IllegalArgumentException("String argument requires JSON string");
            return value;
        }
        return normalize(value, 0);
    }

    private static Object normalize(Object value, int depth) {
        if (depth > 32) throw new IllegalArgumentException("Dubbo argument nesting exceeds 32");
        if (value instanceof JSONArray list) {
            if (list.size() > 1024) throw new IllegalArgumentException("Dubbo list exceeds 1024 items");
            List<Object> normalized = new ArrayList<>(list.size());
            for (Object item : list) normalized.add(normalize(item, depth + 1));
            return normalized;
        }
        if (value instanceof JSONObject object) {
            if (object.size() > 1024) throw new IllegalArgumentException("Dubbo object exceeds 1024 fields");
            Map<String, Object> normalized = new LinkedHashMap<>();
            if (object.containsKey("$class")) {
                String className = required(object.getString("$class"), 256);
                Object payload = object.get("$");
                if (!(payload instanceof JSONObject fields) || object.size() != 2) throw new IllegalArgumentException("Typed DTO wrapper requires object fields");
                normalized.put("class", className);
                for (Map.Entry<String, Object> field : fields.entrySet()) normalized.put(field.getKey(), normalize(field.getValue(), depth + 1));
            } else {
                for (Map.Entry<String, Object> field : object.entrySet()) normalized.put(field.getKey(), normalize(field.getValue(), depth + 1));
            }
            return normalized;
        }
        return value;
    }

    private static String required(String value, int limit) {
        if (value == null || value.isEmpty() || value.length() > limit) throw new IllegalArgumentException("Invalid bounded Dubbo field");
        return value;
    }

    private static JSONObject error(String kind, String className, String message) {
        JSONObject output = new JSONObject();
        output.put("ok", false);
        output.put("kind", kind);
        output.put("class", className == null ? "" : className);
        output.put("message", message == null ? "" : message.substring(0, Math.min(message.length(), 2048)));
        return output;
    }

    private static void write(JSONObject value) throws Exception {
        byte[] bytes = JSON.toJSONBytes(value, JSONWriter.Feature.WriteMapNullValue);
        if (bytes.length > MAX_OUTPUT) bytes = JSON.toJSONBytes(error("worker_error", "OutputLimit", "Dubbo output exceeds 1 MiB"));
        System.out.write(bytes);
        System.out.flush();
    }
}
