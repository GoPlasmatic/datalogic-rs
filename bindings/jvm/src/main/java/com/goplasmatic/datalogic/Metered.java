/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

/**
 * A metered evaluation's result, from
 * {@link Session#evaluateMetered(Rule, String, long)}.
 *
 * @param value result JSON string
 * @param ops   operations charged: one per dispatched node, one per item an
 *              iterator examined, plus what operators charged for the data
 *              they moved
 */
public record Metered(String value, long ops) {}
