// Boundary-benchmark runner for the JVM binding (`runtime: "jvm"`).
//
// Exercises the FFM binding's public API (java.lang.foreign, JDK 22+)
// on the ABI-v2 tiers. Surface used:
//
//   - DataHandle.parse(String json), AutoCloseable
//   - String Session.evaluate(Rule rule, DataHandle data)   (overload)
//   - List<EvalResult> Session.evaluateMany(List<Rule>, DataHandle)
//     — EvalResult is a record (value, errorTag, errorMessage,
//     errorOperator) with isSuccess().
//
// Needs a real JDK on PATH/JAVA_HOME (the macOS system `java` stub has
// no runtime), and the binding's Jackson dependency on the classpath —
// run.sh assembles both via mvn dependency:build-classpath.
//
// Modes: session-evaluate, session-evaluate-data,
// session-evaluate-many-100 (ns_op per evaluation: call/100),
// rule-evaluate, engine-apply-oneshot.
//
// Timing discipline (BINDINGS-OVERHEAD.md appendix): warmup 5,000
// iterations (JIT runtime tier), pilot to ~250 ms per sample, median of
// 5, results consumed into a sink.
//
// Build/run (after `mvn -q -DskipTests package` in bindings/jvm):
//   javac -cp ../../../../bindings/jvm/target/classes runner-jvm/Boundary.java
//   java  -cp ../../../../bindings/jvm/target/classes:runner-jvm \
//         --enable-native-access=ALL-UNNAMED \
//         Boundary <workloads-dir> [--modes=a,b] [--workloads=x,y]
// (Post-rewrite FFM needs --enable-native-access on JDK 24+; the
// pre-rewrite JNA build wants -Djna.library.path=<bindings/c/target/release>.)

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import com.goplasmatic.datalogic.DataHandle;
import com.goplasmatic.datalogic.Engine;
import com.goplasmatic.datalogic.EvalResult;
import com.goplasmatic.datalogic.Rule;
import com.goplasmatic.datalogic.Session;

public final class Boundary {
    static final String RUNTIME = "jvm";
    static final long WARMUP = 5_000; // JIT runtime tier
    static final double TARGET_SAMPLE_NS = 250e6;
    static final double PILOT_MIN_NS = 10e6;
    static final int SAMPLES = 5;
    static final int MANY_N = 100;

    static long globalSink = 0;

    interface Batch {
        long run(long n);
    }

    /** Warmup + pilot + median-of-5; returns ns per batch iteration. */
    static double measure(Batch batch) {
        globalSink += batch.run(WARMUP);

        long n = 32;
        double perOp;
        while (true) {
            long t0 = System.nanoTime();
            globalSink += batch.run(n);
            long elapsed = System.nanoTime() - t0;
            if (elapsed >= PILOT_MIN_NS) {
                perOp = (double) elapsed / (double) n;
                break;
            }
            n *= 2;
        }

        long iters = Math.max(1, Math.round(TARGET_SAMPLE_NS / perOp));
        double[] samples = new double[SAMPLES];
        for (int s = 0; s < SAMPLES; s++) {
            long t0 = System.nanoTime();
            globalSink += batch.run(iters);
            samples[s] = (double) (System.nanoTime() - t0) / (double) iters;
        }
        Arrays.sort(samples);
        return samples[SAMPLES / 2];
    }

    static void emit(String mode, String workload, double nsOp) {
        System.out.printf(
                "{\"runtime\": \"%s\", \"mode\": \"%s\", \"workload\": \"%s\", \"ns_op\": %.3f}%n",
                RUNTIME, mode, workload, nsOp);
    }

    static void verify(String mode, String workload, String got, String expected) {
        if (!got.equals(expected)) {
            System.err.printf(
                    "runner-jvm: verification failed for mode=%s workload=%s%n  expected: %s%n  got:      %s%n",
                    mode, workload, expected, got);
            System.exit(1);
        }
    }

    static boolean selected(List<String> filter, String name) {
        return filter == null || filter.contains(name);
    }

    public static void main(String[] args) throws IOException {
        String dir = null;
        List<String> modeFilter = null;
        List<String> workloadFilter = null;
        for (String arg : args) {
            if (arg.startsWith("--modes=")) {
                modeFilter = Arrays.asList(arg.substring("--modes=".length()).split(","));
            } else if (arg.startsWith("--workloads=")) {
                workloadFilter = Arrays.asList(arg.substring("--workloads=".length()).split(","));
            } else {
                dir = arg;
            }
        }
        if (dir == null) {
            System.err.println("usage: Boundary <workloads-dir> [--modes=a,b] [--workloads=x,y]");
            System.exit(1);
        }

        try (Engine engine = new Engine()) {
            for (String name : new String[] {"simple", "eligibility", "array100"}) {
                if (!selected(workloadFilter, name)) continue;

                String ruleJson = Files.readString(Path.of(dir, name + ".rule.json"));
                String dataJson = Files.readString(Path.of(dir, name + ".data.json"));
                String expected = Files.readString(Path.of(dir, name + ".expected.json"));

                try (Rule rule = engine.compile(ruleJson);
                        Session session = engine.openSession();
                        // v2: parse-once data handle.
                        DataHandle dataHandle = DataHandle.parse(dataJson)) {

                    // 100 identical rules, compiled separately (a rule-set
                    // of identical rules — separate compiles so the batch
                    // doesn't flatter one hot compiled tree).
                    List<Rule> manyRules = new ArrayList<>(MANY_N);
                    for (int i = 0; i < MANY_N; i++) manyRules.add(engine.compile(ruleJson));

                    record Mode(String name, Runnable verify, Batch batch, double perCallEvals) {}
                    List<Mode> modes = new ArrayList<>();

                    modes.add(new Mode("session-evaluate",
                            () -> verify("session-evaluate", name,
                                    session.evaluate(rule, dataJson), expected),
                            n -> {
                                long sink = 0;
                                for (long i = 0; i < n; i++)
                                    sink += session.evaluate(rule, dataJson).length();
                                return sink;
                            }, 1.0));

                    modes.add(new Mode("session-evaluate-data",
                            () -> verify("session-evaluate-data", name,
                                    session.evaluate(rule, dataHandle), expected),
                            n -> {
                                long sink = 0;
                                for (long i = 0; i < n; i++)
                                    sink += session.evaluate(rule, dataHandle).length();
                                return sink;
                            }, 1.0));

                    modes.add(new Mode("session-evaluate-many-100",
                            () -> {
                                // v2: N rules x one data handle; per-item outcomes.
                                for (EvalResult r : session.evaluateMany(manyRules, dataHandle)) {
                                    if (!r.isSuccess()) {
                                        System.err.println("runner-jvm: many item failed: "
                                                + r.errorTag() + ": " + r.errorMessage());
                                        System.exit(1);
                                    }
                                    verify("session-evaluate-many-100", name, r.value(), expected);
                                }
                            },
                            n -> {
                                long sink = 0;
                                for (long i = 0; i < n; i++) {
                                    List<EvalResult> results = session.evaluateMany(manyRules, dataHandle);
                                    sink += results.get(0).value().length()
                                            + results.get(MANY_N - 1).value().length();
                                }
                                return sink;
                            }, MANY_N));

                    modes.add(new Mode("rule-evaluate",
                            () -> verify("rule-evaluate", name, rule.evaluate(dataJson), expected),
                            n -> {
                                long sink = 0;
                                for (long i = 0; i < n; i++)
                                    sink += rule.evaluate(dataJson).length();
                                return sink;
                            }, 1.0));

                    modes.add(new Mode("engine-apply-oneshot",
                            () -> verify("engine-apply-oneshot", name,
                                    engine.apply(ruleJson, dataJson), expected),
                            n -> {
                                long sink = 0;
                                for (long i = 0; i < n; i++)
                                    sink += engine.apply(ruleJson, dataJson).length();
                                return sink;
                            }, 1.0));

                    for (Mode m : modes) {
                        if (!selected(modeFilter, m.name())) continue;
                        m.verify().run();
                        emit(m.name(), name, measure(m.batch()) / m.perCallEvals());
                    }

                    for (Rule r : manyRules) r.close();
                }
            }
        }

        System.err.println("runner-jvm: sink=" + globalSink);
    }
}
