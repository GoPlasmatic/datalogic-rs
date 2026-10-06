/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import java.lang.foreign.MemorySegment;
import java.lang.invoke.MethodHandle;
import java.lang.ref.Cleaner;
import java.util.concurrent.atomic.AtomicReference;

/**
 * One owned native handle, freed exactly once: by the owner's explicit
 * {@code close()}, or by a shared {@link Cleaner} once the owner becomes
 * unreachable without one.
 *
 * <p>The free is an atomic {@code getAndSet(null)}, so concurrent
 * {@code close()} calls (and a close racing the Cleaner) free the handle
 * once. A close racing another call that is still using the handle is
 * not made safe by this; the owners document that.
 *
 * <p>Owners must keep themselves reachable (with
 * {@link java.lang.ref.Reference#reachabilityFence}) until every native
 * call that received {@link #get()} has returned: otherwise the Cleaner
 * may free the handle while native code still uses it.
 */
final class NativeHandle {
    // One daemon thread for every handle in the process.
    private static final Cleaner CLEANER = Cleaner.create();

    private final Release release;
    private final Cleaner.Cleanable cleanable;
    private final String owner;

    /**
     * Take ownership of {@code segment} for {@code ownerObject}: it is
     * freed with {@code free} (a {@code (MemorySegment)void} downcall) on
     * {@link #close()} or once {@code ownerObject} is unreachable.
     * {@code ownerName} names the owner in the "is closed" message.
     */
    NativeHandle(Object ownerObject, MemorySegment segment, MethodHandle free, String ownerName) {
        this.release = new Release(segment, free);
        this.cleanable = CLEANER.register(ownerObject, release);
        this.owner = ownerName;
    }

    /** The live handle; throws {@link IllegalStateException} once closed. */
    MemorySegment get() {
        MemorySegment s = release.segment.get();
        if (s == null) throw new IllegalStateException(owner + " is closed");
        return s;
    }

    /** Free the handle now, unless it already was. Idempotent and thread-safe. */
    void close() {
        cleanable.clean();
    }

    /**
     * The cleaning action. It must not reference the owner, or the owner
     * would never become unreachable.
     */
    private static final class Release implements Runnable {
        final AtomicReference<MemorySegment> segment;
        private final MethodHandle free;

        Release(MemorySegment segment, MethodHandle free) {
            this.segment = new AtomicReference<>(segment);
            this.free = free;
        }

        @Override
        public void run() {
            MemorySegment s = segment.getAndSet(null);
            if (s == null) return;
            try {
                free.invokeExact(s);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
        }
    }
}
