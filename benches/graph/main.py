import os
import re
from collections import defaultdict
from subprocess import run
import matplotlib.pyplot as plt


def convert_to_ms(value_str):
    value_str = value_str.strip()
    match = re.match(r'([\d.]+)\s*([µu]s|ms|s)\b', value_str)
    if match:
        value = float(match.group(1))
        unit = match.group(2)

        if unit in ['µs', 'us']:
            return value / 1000
        elif unit == 'ms':
            return value
        elif unit == 's':
            return value * 1000

    return None


def parse_benchmarks(text):
    results = []
    current_benchmark = None

    lines = text.strip().split('\n')

    for line in lines:
        benchmark_match = re.search(r'[├╰]─\s+(\w+)\s+│', line)
        if benchmark_match and '│' in line and not re.search(r'[├╰]─\s+\d+\s+', line):
            current_benchmark = benchmark_match.group(1)
            continue

        if current_benchmark and re.match(r'\s*[│\s]*[├╰]─\s+\d+\s+', line):
            normalized_line = line.lstrip('│')
            parts = normalized_line.split('│')
            if len(parts) >= 5:
                thread_match = re.search(r'(\d+)', parts[0])
                if thread_match:
                    thread_count = int(thread_match.group(1))
                    mean_runtime = parts[3].strip()
                    mean_ms = convert_to_ms(mean_runtime)

                    if mean_ms is not None:
                        results.append({
                            'benchmark': current_benchmark,
                            'thread_count': thread_count,
                            'mean_runtime': mean_runtime,
                            'mean_ms': mean_ms
                        })

    return results

if __name__ == '__main__':
    os.chdir("../..")
    data = run("cargo bench --profile release", capture_output=True, shell=True, text=True)

    results = parse_benchmarks(data.stdout)

    grouped = defaultdict(lambda: {'threads': [], 'means': []})
    for result in results:
        benchmark = result['benchmark']
        grouped[benchmark]['threads'].append(result['thread_count'])
        grouped[benchmark]['means'].append(result['mean_ms'])

    plt.figure(figsize=(12, 7))

    colors = ['#2E86AB', '#A23B72']
    markers = ['o', 's']

    for idx, (benchmark, data) in enumerate(sorted(grouped.items())):
        label = benchmark.replace('_', ' ').title()

        plt.plot(data['threads'], data['means'],
                 marker=markers[idx],
                 linewidth=2,
                 markersize=8,
                 color=colors[idx],
                 label=label)

        y_offset = -15 if idx == 0 else 10
        for thread_count, mean_time in zip(data['threads'], data['means']):
            total_ops_k = thread_count
            plt.annotate(f'{total_ops_k}k ops',
                        xy=(thread_count, mean_time),
                        xytext=(0, y_offset),
                        textcoords='offset points',
                        fontsize=8,
                        alpha=0.75,
                        color=colors[idx],
                        ha='center')

    plt.xlabel('Thread Count', fontsize=12, fontweight='bold')
    plt.ylabel('Mean Runtime (ms)', fontsize=12, fontweight='bold')
    plt.title('Benchmark Performance: Mean Runtime vs Thread Count', fontsize=14, fontweight='bold')
    plt.legend(fontsize=11, loc='upper left')
    plt.grid(True, alpha=0.3, linestyle='--')
    plt.xticks(sorted(grouped[list(grouped.keys())[0]]['threads']))

    plt.tight_layout()
    plt.savefig('benchmark_comparison.png', dpi=300, bbox_inches='tight')
    plt.show()