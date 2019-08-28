#!/bin/bash
cutadapt --times 3 --overlap 5 -f fastq --interleaved -g spacer=^TTTCTTATATGGG -a R2_rc=AGATCGGAAGAGCACACGTCTGAACTCCAGTCAC -a P7_rc=ATCTCGTATGCCGTCTTCTGCTTG -a polyA=AAAAAAAAAAAAAAAAAAAA -a rt_primer_rc=ATGTACTCTGCGTTGATACCACTGCTT -A spacer_rc=CCCATATAAGAAA -A R1_rc=AGATCGGAAGAGCGTCGTGTAGGGAAAGAGTGT -A P5_rc=AGATCTCGGTGGTCGCCGTATCATT -G polyT=TTTTTTTTTTTTTTTTTTTT -G rt_primer=XAAGCAGTGGTATCAACGCAGAGTACAT $1 > /dev/null 2>&1