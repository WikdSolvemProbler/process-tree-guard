# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Draft quiet-host scoring for issue #7. No benchmark launch entry point."""

from dataclasses import dataclass
from statistics import median

INTERVALS = 12
SECONDS = 5
MIB = 1024 * 1024
MIN_AVAILABLE = 4096 * MIB
MEDIAN_CPU = 10.0
OUTSIDE_MAX_CPU = 15.0
DURING_MAX_CPU = 25.0
DURING_PAIRED_CPU = 15.0
MAX_READ_NS = 10_000_000
MAX_WAKE_DIFFERENCE = 2_500_000
NS_PER_SECOND = 1_000_000_000
MAX_OBSERVER_CPU = 0.1
# One 100ns awake-clock representation tick, used only for clock bracketing.
# This supplies no tolerance for system or Job CPU accounting discrepancies.
AWAKE_QPC_TOLERANCE_NS = 100


@dataclass(frozen=True)
class Cpu:
    idle: int
    kernel: int
    user: int

    @property
    def total(self):
        return self.kernel + self.user

    @property
    def busy(self):
        return self.total - self.idle


@dataclass(frozen=True)
class Job:
    kernel: int
    user: int
    total_processes: int
    active_processes: int

    @property
    def total(self):
        return self.kernel + self.user


@dataclass(frozen=True)
class ReadBounds:
    job_before_begin: int
    job_before_end: int
    awake_before_begin: int
    awake_before_end: int
    system_begin: int
    system_end: int
    awake_after_begin: int
    awake_after_end: int
    job_after_begin: int
    job_after_end: int


@dataclass(frozen=True)
class Sample:
    monotonic_ns: int
    awake_ticks: int
    interrupt_ticks: int
    observer_ns: int
    read_ns: int
    groups: int
    processors: int
    available_bytes: int
    ac_line: int
    saver: int
    schema: int
    system: Cpu
    system_awake_before: int
    system_awake_after: int
    job_before: Job | None
    job_after: Job | None
    calls: ReadBounds
    native_errors: tuple[str, ...]

    @property
    def job(self):
        return self.job_after


def cpu_ordered(first, second):
    return (first.idle <= second.idle and first.kernel <= second.kernel
            and first.user <= second.user and first.busy <= second.busy)


def uint(value, bits=64):
    return type(value) is int and 0 <= value < 2**bits


def valid_cpu(cpu):
    return isinstance(cpu, Cpu) and all(uint(value) for value in (cpu.idle, cpu.kernel, cpu.user))


def valid_job(job):
    return (isinstance(job, Job) and uint(job.kernel, 63) and uint(job.user, 63)
            and uint(job.total_processes, 32) and uint(job.active_processes, 32)
            and job.active_processes <= job.total_processes)


def shape_errors(sample, *, during=False):
    scalar_limits = (
        (sample.monotonic_ns, 63), (sample.awake_ticks, 64), (sample.interrupt_ticks, 64),
        (sample.observer_ns, 63), (sample.read_ns, 63), (sample.groups, 16),
        (sample.processors, 32), (sample.available_bytes, 64), (sample.ac_line, 8),
        (sample.saver, 8), (sample.system_awake_before, 64), (sample.system_awake_after, 64),
    )
    if (any(not uint(value, bits) for value, bits in scalar_limits)
            or not isinstance(sample.calls, ReadBounds)
            or any(not uint(value, 63) for value in vars(sample.calls).values())
            or not valid_cpu(sample.system) or type(sample.schema) is not int
            or not isinstance(sample.native_errors, tuple)
            or any(not isinstance(error, str) for error in sample.native_errors)
            or (during and not all(valid_job(job) for job in (sample.job_before, sample.job_after)))
            or (not during and (sample.job_before is not None or sample.job_after is not None))):
        return ["native_sample_shape"]
    return []


def sample_errors(sample, *, during=False):
    errors = shape_errors(sample, during=during)
    if errors:
        return errors
    if sample.schema != 2 or sample.native_errors:
        errors.append("native_sample")
    calls = sample.calls
    sequence = [
        calls.awake_before_begin, calls.awake_before_end,
        calls.system_begin, calls.system_end,
        calls.awake_after_begin, calls.awake_after_end,
    ]
    if during:
        sequence = [calls.job_before_begin, calls.job_before_end, *sequence,
                    calls.job_after_begin, calls.job_after_end]
        if sample.job_before is None or sample.job_after is None:
            errors.append("missing_job_cpu")
        elif (sample.job_before.kernel > sample.job_after.kernel
              or sample.job_before.user > sample.job_after.user
              or sample.job_before.total_processes > sample.job_after.total_processes):
            errors.append("job_counter_order")
    elif (calls.job_before_begin, calls.job_before_end,
          calls.job_after_begin, calls.job_after_end) != (0, 0, 0, 0):
        errors.append("unexpected_job_call")
    if (sequence != sorted(sequence) or sample.monotonic_ns > sequence[0]
            or sequence[-1] > sample.monotonic_ns + sample.read_ns
            or not sample.awake_ticks <= sample.system_awake_before <= sample.system_awake_after):
        errors.append("counter_call_bracket")
    if sample.system.idle > sample.system.kernel:
        errors.append("system_cpu_shape")
    if ((sample.system_awake_after - sample.awake_ticks) * 100
            > sample.read_ns + AWAKE_QPC_TOLERANCE_NS):
        errors.append("awake_qpc_span")
    if not 0 <= sample.read_ns <= MAX_READ_NS:
        errors.append("counter_read_span")
    return errors


def window(first, last, *, during=False):
    shapes = shape_errors(first, during=during) + shape_errors(last, during=during)
    if shapes:
        return {
            "errors": shapes, "residual_cpu_upper_pct": None,
            "capacity_lower_ticks": None, "system_total_observed_ticks": None,
            "awake_capacity_lower_ticks": None, "qpc_capacity_lower_ticks": None,
            "system_busy_observed_ticks": None, "job_cpu_ticks": None,
            "raw_residual_ticks": None, "sampler_cpu_ns": None,
        }
    errors = []
    elapsed = last.monotonic_ns - first.monotonic_ns
    awake = last.awake_ticks - first.awake_ticks
    interrupt = last.interrupt_ticks - first.interrupt_ticks
    if not 4.5 * NS_PER_SECOND <= elapsed <= 7.5 * NS_PER_SECOND:
        errors.append("sample_gap")
    if awake <= 0 or interrupt <= 0 or abs(interrupt - awake) > MAX_WAKE_DIFFERENCE:
        errors.append("awake_clock_discontinuity")
    if any(not 0 <= sample.read_ns <= MAX_READ_NS for sample in (first, last)):
        errors.append("counter_read_span")
    if first.groups != 1 or last.groups != 1 or not 1 <= first.processors <= 64 \
            or first.processors != last.processors:
        errors.append("processor_group_or_count")
    if min(first.available_bytes, last.available_bytes) < MIN_AVAILABLE:
        errors.append("physical_memory")
    if (first.ac_line, last.ac_line, first.saver, last.saver) != (1, 1, 0, 0):
        errors.append("power_state")

    for sample in (first, last):
        errors.extend(sample_errors(sample, during=during))
    if first.monotonic_ns + first.read_ns > last.monotonic_ns:
        errors.append("counter_call_overlap")
    if not cpu_ordered(first.system, last.system):
        errors.append("system_cpu_counter_order")
    observed_total = last.system.total - first.system.total
    busy_upper = last.system.busy - first.system.busy
    awake_inner = last.system_awake_before - first.system_awake_after
    # QPC timestamps are integer nanoseconds. Floor once to 100ns CPU units.
    qpc_inner = (last.calls.system_begin - first.calls.system_end) // 100
    awake_capacity = awake_inner * first.processors
    qpc_capacity = qpc_inner * first.processors
    capacity = min(awake_capacity, qpc_capacity)
    if awake_inner <= 0 or qpc_inner <= 0:
        errors.append("capacity_clock_order")
    if capacity <= 0 or not 0.9 * capacity <= observed_total <= 1.1 * capacity:
        errors.append("system_counter_capacity")
    observer = last.observer_ns - first.observer_ns
    if observer < 0 or (capacity > 0 and observer > capacity * MAX_OBSERVER_CPU):
        errors.append("observer_cpu")
    job_delta = 0
    if during:
        jobs = [first.job_before, first.job_after, last.job_before, last.job_after]
        if any(job is None for job in jobs):
            errors.append("missing_job_cpu")
        elif (any(min(job.kernel, job.user, job.total_processes, job.active_processes) < 0
                  or job.active_processes > job.total_processes for job in jobs)
              or any(left.kernel > right.kernel or left.user > right.user
                     or left.total_processes > right.total_processes
                     for left, right in zip(jobs, jobs[1:]))):
            errors.append("job_counter_order")
        else:
            job_delta = max(0, last.job_before.total - first.job_after.total)
    elif any(job is not None for job in (first.job_before, first.job_after,
                                        last.job_before, last.job_after)):
        errors.append("unexpected_job_cpu")

    raw_residual = busy_upper - job_delta
    if raw_residual < 0:
        errors.append("job_exceeds_system_busy")
    value = None if capacity <= 0 or raw_residual < 0 else 100 * raw_residual / capacity
    return {
        "errors": errors, "residual_cpu_upper_pct": value,
        "capacity_lower_ticks": capacity, "system_total_observed_ticks": observed_total,
        "awake_capacity_lower_ticks": awake_capacity,
        "qpc_capacity_lower_ticks": qpc_capacity,
        "system_busy_observed_ticks": busy_upper,
        "job_cpu_ticks": job_delta, "raw_residual_ticks": raw_residual,
        "sampler_cpu_ns": observer,
    }


def verdict(windows, *, during=False):
    reasons = [f"window_{index}:{reason}"
               for index, row in enumerate(windows, 1) for reason in row["errors"]]
    count = len(windows)
    if (not during and count != INTERVALS) or (during and count < INTERVALS):
        reasons.append("coverage")
    values = [row["residual_cpu_upper_pct"] for row in windows]
    if any(value is None for value in values):
        reasons.append("missing_cpu")
    elif values:
        if median(values) > MEDIAN_CPU:
            reasons.append("median_cpu")
        if max(values) > (DURING_MAX_CPU if during else OUTSIDE_MAX_CPU):
            reasons.append("maximum_cpu")
        if during and any(min(a, b) > DURING_PAIRED_CPU for a, b in zip(values, values[1:])):
            reasons.append("adjacent_high_cpu")
    return {"candidate_pass": not reasons, "reasons": reasons}
