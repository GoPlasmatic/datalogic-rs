// custom-operator: register a Java `double` operator and call it from a rule.
// Custom operators receive their pre-evaluated arguments as a JSON-array
// string and return a JSON-value string. Built-in names always win.
//
// Run from bindings/jvm/ (needs JDK 22+, and the C ABI built once:
// `cargo build --release --manifest-path ../c/Cargo.toml`):
//   mvn -q compile dependency:build-classpath -Dmdep.outputFile=target/cp.txt
//   java --enable-native-access=ALL-UNNAMED \
//        -cp "target/classes:$(cat target/cp.txt)" \
//        -Ddatalogic.library.path=../c/target/release examples/CustomOperator.java

import com.fasterxml.jackson.databind.ObjectMapper;
import com.goplasmatic.datalogic.Engine;

public class CustomOperator {
    public static void main(String[] args) {
        ObjectMapper mapper = new ObjectMapper();

        try (Engine engine = Engine.builder()
                .addOperator("double", argsJson -> {
                    int n = mapper.readTree(argsJson).get(0).asInt();
                    return String.valueOf(n * 2);
                })
                .build()) {
            System.out.println(engine.apply("{\"double\": [21]}", "{}")); // 42
        }
    }
}
