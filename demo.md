# Quarto Term Demo


## Basic Usage

Simple commands in a persistent session:

```console
$ echo 'Hello, world!'
Hello, world!
```

Variables persist across cells:

```console
$ echo $GREETING
Hello from quarto-term
```

## Working with Files

```console
$ echo "line 1" > /tmp/demo.txt

$ echo "line 2" >> /tmp/demo.txt

$ echo "line 3" >> /tmp/demo.txt

$ cat /tmp/demo.txt
line 1
line 2
line 3
```

## Piping and Arithmetic

```console
$ seq 1 5 | paste -sd+ | bc
15
```

## ANSI Colors

Commands that produce colored output:

```console
$ printf '\033[31mred\033[0m \033[32mgreen\033[0m \033[34mblue\033[0m\n'
red green blue
```

## Per-line Options

### Interrupting a long-running command

Send `sleep 100`, then after half a second send Ctrl-C:

```console
$ sleep 100
^C
```

### Navigating with arrow keys

```console
$ echo "first command"
first command

$ echo "first command"
first command
```

## Fullscreen Capture

Capture the entire terminal screen (useful for TUI apps like htop, top,
vim):

```text

    0[||||||||70.7%]   4[         0.0%]   7[          0.0%] 11[||||     16.0%]
    1[||||||||69.8%]   5[         0.0%]   8[          0.0%] 12[||||     15.3%]
    2[||||||||74.2%]   6[         0.0%]   9[||        8.0%] 13[|         0.7%]
    3[||||||||72.0%]                     10[||        5.4%]
  Mem[|||||||||||||||||||||19.6G/24.0G] Tasks: 664, 5157 thr, 0 kthr; 5 runnin
  Swp[|||||||||||||||||||||9.46G/11.0G] Load average: 6.71 6.29 6.30
                                        Uptime: 13 days, 11:03:36

  [Main]
  PID USER       PRI  NI  VIRT   RES S  CPU%▽MEM%   TIME+  Command
86489 jeroen      17   0  421G 90832 R   2.0  0.4  3h11:00 /Applications/Adobe I
16870 jeroen      24   0 1577G  454M S   1.0  1.9  1h36:41 /Applications/Obsidia
10921 jeroen      17   0  416G  126M R   1.0  0.5  1h22:53 /Applications/Elgato
65086 jeroen      26   0  420G  442M R   0.3  1.8  0:27.22 claude --dangerously-
16868 jeroen      17   0  447G 38992 S   0.1  0.2 11:03.52 /Applications/Obsidia
56514 jeroen      24   0  419G 55776 S   0.1  0.2  8:06.99 /Applications/DaVinci
27311 jeroen      17   0  415G 63712 R   0.1  0.3  4:49.73 /Applications/iTerm.a
34167 jeroen      17   0  421G  157M S   0.1  0.6 21:05.89 /Applications/zoom.us
27185 jeroen      17   0  396G 50096 S   0.1  0.2  0:46.45 /Applications/kitty.a
 1794 jeroen      17   0  419G 13472 S   0.0  0.1  9:31.86 osascript /Users/jero
76651 jeroen      17   0  416G  119M S   0.0  0.5  2:51.23 /Applications/WhatsAp
34213 jeroen      24   0  415G 30128 S   0.0  0.1  4:17.63 /Applications/zoom.us
F1Help  F2Setup F3SearchF4FilterF5Tree  F6SortByF7Nice -F8Nice +F9Kill  F10Quit
```

## Cleanup
