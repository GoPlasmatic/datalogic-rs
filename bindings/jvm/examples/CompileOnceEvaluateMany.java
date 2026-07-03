// compile-once-evaluate-many: compile a rule once, evaluate it against
// ~100k payloads, and print the last result plus a rough per-evaluation
// cost — first with JSON strings, then on the hot path with pre-parsed
// DataHandles + a Session, and finally as one evaluateBatch call.
//
// Run from bindings/jvm/ (needs JDK 22+, and the C ABI built once:
// `cargo build --release --manifest-path ../c/Cargo.toml`):
//   mvn -q compile dependency:build-classpath -Dmdep.outputFile=target/cp.txt
//   java --enable-native-access=ALL-UNNAMED \
//        -cp "target/classes:$(cat target/cp.txt)" \
//        -Ddatalogic.library.path=../c/target/release examples/CompileOnceEvaluateMany.java

import com.goplasmatic.datalogic.DataHandle;
import com.goplasmatic.datalogic.Engine;
import com.goplasmatic.datalogic.EvalResult;
import com.goplasmatic.datalogic.Rule;
import com.goplasmatic.datalogic.Session;

import java.util.ArrayList;
import java.util.List;

public class CompileOnceEvaluateMany {
    private static final int ITERATIONS = 100_000;
    private static final int PAYLOADS = 100;

    public static void main(String[] args) {
        String logic = "{\"*\": [{\"var\": \"price\"}, {\"-\": [1, {\"var\": \"discount\"}]}]}";

        try (Engine engine = new Engine(); Rule rule = engine.compile(logic)) {

            // Tier 1: JSON strings in, JSON strings out.
            String last = null;
            long start = System.nanoTime();
            for (int i = 0; i < ITERATIONS; i++) {
                last = rule.evaluate("{\"price\": " + (100 + i % PAYLOADS) + ", \"discount\": 0.2}");
            }
            long elapsedNs = System.nanoTime() - start;
            System.out.println("last result: " + last);
            System.out.println(ITERATIONS + " string evaluations, ~" + (elapsedNs / ITERATIONS) + " ns/op");

            // Tier 2: parse each distinct payload once, evaluate many
            // times via a session — zero JSON parse work per call.
            List<DataHandle> payloads = new ArrayList<>(PAYLOADS);
            for (int p = 0; p < PAYLOADS; p++) {
                payloads.add(DataHandle.parse("{\"price\": " + (100 + p) + ", \"discount\": 0.2}"));
            }
            try (Session session = engine.openSession()) {
                start = System.nanoTime();
                for (int i = 0; i < ITERATIONS; i++) {
                    last = session.evaluate(rule, payloads.get(i % PAYLOADS));
                }
                elapsedNs = System.nanoTime() - start;
                System.out.println(ITERATIONS + " data-handle evaluations, ~" + (elapsedNs / ITERATIONS) + " ns/op");

                // Tier 3: one rule x all payloads in a single native call.
                List<EvalResult> batch = session.evaluateBatch(rule, payloads);
                System.out.println("batch of " + batch.size() + ", first=" + batch.get(0).value()
                        + ", last=" + batch.get(batch.size() - 1).value());
            } finally {
                payloads.forEach(DataHandle::close);
            }
        }
    }
}
