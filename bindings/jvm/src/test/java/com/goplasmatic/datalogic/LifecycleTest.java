/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import org.junit.jupiter.api.Test;

import java.lang.foreign.MemorySegment;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.*;

class LifecycleTest {

    // ---- NativeHandle, with a counting stand-in for the native free ----

    private static final MethodHandle COUNTING_FREE;

    static {
        try {
            COUNTING_FREE = MethodHandles.lookup().findStatic(LifecycleTest.class, "countingFree",
                    MethodType.methodType(void.class, AtomicInteger.class, MemorySegment.class));
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    @SuppressWarnings("unused") // invoked through COUNTING_FREE
    private static void countingFree(AtomicInteger frees, MemorySegment segment) {
        frees.incrementAndGet();
    }

    @Test
    void native_handle_close_frees_once_and_then_refuses_use() {
        AtomicInteger frees = new AtomicInteger();
        Object owner = new Object();
        NativeHandle handle = new NativeHandle(owner, MemorySegment.ofAddress(8),
                COUNTING_FREE.bindTo(frees), "Thing");
        assertEquals(8, handle.get().address());
        handle.close();
        handle.close();
        assertEquals(1, frees.get());
        IllegalStateException ex = assertThrows(IllegalStateException.class, handle::get);
        assertEquals("Thing is closed", ex.getMessage());
    }

    @Test
    void native_handle_close_from_many_threads_frees_once() throws InterruptedException {
        for (int round = 0; round < 20; round++) {
            AtomicInteger frees = new AtomicInteger();
            NativeHandle handle = new NativeHandle(new Object(), MemorySegment.ofAddress(8),
                    COUNTING_FREE.bindTo(frees), "Thing");
            closeConcurrently(handle::close);
            assertEquals(1, frees.get());
        }
    }

    @Test
    void native_handle_of_an_unreachable_owner_is_freed_by_the_cleaner() throws InterruptedException {
        AtomicInteger frees = new AtomicInteger();
        registerAndDrop(frees);
        for (int i = 0; i < 200 && frees.get() == 0; i++) {
            System.gc();
            Thread.sleep(10);
        }
        assertEquals(1, frees.get());
    }

    private static void registerAndDrop(AtomicInteger frees) {
        Object owner = new Object();
        new NativeHandle(owner, MemorySegment.ofAddress(8), COUNTING_FREE.bindTo(frees), "Thing");
    }

    // ---- the real handles ---------------------------------------------

    @Test
    void close_from_many_threads_is_safe_for_every_handle() throws InterruptedException {
        for (int round = 0; round < 20; round++) {
            Engine engine = new Engine();
            Rule rule = engine.compile("{\"var\":\"x\"}");
            DataHandle data = DataHandle.parse("{\"x\":1}");
            Session session = engine.openSession();
            TracedSession traced = engine.openTracedSession();
            closeConcurrently(() -> {
                traced.close();
                session.close();
                data.close();
                rule.close();
                engine.close();
            });
            assertThrows(IllegalStateException.class, () -> rule.evaluate("{}"));
        }
    }

    @Test
    void unclosed_handles_and_builders_are_released_without_crashing() throws InterruptedException {
        for (int i = 0; i < 20; i++) {
            dropUnclosedHandles();
        }
        for (int i = 0; i < 5; i++) {
            System.gc();
            Thread.sleep(10);
        }
        try (Engine engine = Engine.builder().addOperator("one", args -> "1").build()) {
            assertEquals("1", engine.apply("{\"one\":[]}", "{}"));
        }
    }

    private static void dropUnclosedHandles() {
        Engine engine = Engine.builder().addOperator("one", args -> "1").build();
        Rule rule = engine.compile("{\"one\":[]}");
        assertEquals("1", rule.evaluate("{}"));
        engine.openSession();
        engine.openTracedSession();
        DataHandle.parse("{}");
        // A builder abandoned after a failed setter.
        assertThrows(EvaluateException.class, () -> Engine.builder()
                .addOperator("one", args -> "1")
                .setConfigJson("{\"no_such_key\":1}"));
    }

    /** Run {@code action} from eight threads released at the same moment. */
    private static void closeConcurrently(Runnable action) throws InterruptedException {
        CountDownLatch start = new CountDownLatch(1);
        List<Thread> threads = new ArrayList<>();
        List<Throwable> failures = new ArrayList<>();
        for (int i = 0; i < 8; i++) {
            Thread t = new Thread(() -> {
                try {
                    start.await();
                    action.run();
                } catch (Throwable e) {
                    synchronized (failures) {
                        failures.add(e);
                    }
                }
            });
            threads.add(t);
            t.start();
        }
        start.countDown();
        for (Thread t : threads) {
            t.join();
        }
        assertTrue(failures.isEmpty(), () -> "close threw: " + failures);
    }
}
