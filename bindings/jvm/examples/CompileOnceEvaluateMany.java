// compile-once-evaluate-many: compile a rule once, evaluate it against
// ~100k payloads, and print the last result plus a rough per-evaluation cost.
//
// Run from bindings/jvm/ (needs the C ABI built once:
// `cargo build --release --manifest-path ../c/Cargo.toml`):
//   mvn -q compile dependency:build-classpath -Dmdep.outputFile=target/cp.txt
//   java -cp "target/classes:$(cat target/cp.txt)" \
//        -Djna.library.path=../c/target/release examples/CompileOnceEvaluateMany.java

import com.goplasmatic.datalogic.Engine;
import com.goplasmatic.datalogic.Rule;

public class CompileOnceEvaluateMany {
    private static final int ITERATIONS = 100_000;

    public static void main(String[] args) {
        String logic = "{\"*\": [{\"var\": \"price\"}, {\"-\": [1, {\"var\": \"discount\"}]}]}";

        try (Engine engine = new Engine(); Rule rule = engine.compile(logic)) {
            String last = null;
            long start = System.nanoTime();
            for (int i = 0; i < ITERATIONS; i++) {
                last = rule.evaluate("{\"price\": " + (100 + i % 100) + ", \"discount\": 0.2}");
            }
            long elapsedNs = System.nanoTime() - start;

            System.out.println("last result: " + last);
            System.out.println(ITERATIONS + " evaluations, ~" + (elapsedNs / ITERATIONS) + " ns/op");
        }
    }
}
