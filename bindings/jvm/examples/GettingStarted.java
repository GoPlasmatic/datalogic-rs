// getting-started: one-shot JSONLogic evaluation with the datalogic JVM binding.
//
// Run from bindings/jvm/ (needs the C ABI built once:
// `cargo build --release --manifest-path ../c/Cargo.toml`):
//   mvn -q compile dependency:build-classpath -Dmdep.outputFile=target/cp.txt
//   java -cp "target/classes:$(cat target/cp.txt)" \
//        -Djna.library.path=../c/target/release examples/GettingStarted.java

import com.goplasmatic.datalogic.Engine;

public class GettingStarted {
    public static void main(String[] args) {
        String rule = "{\"and\": [{\">=\": [{\"var\": \"age\"}, 18]},"
                + " {\"==\": [{\"var\": \"status\"}, \"active\"]}]}";
        String data = "{\"age\": 25, \"status\": \"active\"}";

        try (Engine engine = new Engine()) {
            System.out.println(engine.apply(rule, data)); // true
        }
    }
}
