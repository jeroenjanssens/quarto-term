# Quarto Term Demo


## Basic Usage

Simple commands in a persistent session:

```
$ echo 'Hello, world!'
Hello, world!
```

Variables persist across cells:

```
$ echo $GREETING
Hello from quarto-term
```

## Working with Files

```
$ echo "line 1" > /tmp/demo.txt

$ echo "line 2" >> /tmp/demo.txt

$ echo "line 3" >> /tmp/demo.txt

$ cat /tmp/demo.txt
line 1
line 2
line 3
```

## Piping and Arithmetic

```
$ seq 1 5 | paste -sd+ | bc
15
```

## ANSI Colors

Commands that produce colored output:

```
$ printf '\033[31mred\033[0m \033[32mgreen\033[0m \033[34mblue\033[0m\n'
red green blue
```

## Per-line Options

### Interrupting a long-running command

Send `sleep 100`, then after half a second send Ctrl-C:

```
$ sleep 100
^C
```

### Navigating with arrow keys

```
$ echo "first command"
first command

$ echo "first command"
first command
```

## Fullscreen Capture

Capture the entire terminal screen (useful for TUI apps like htop, top,
vim):

```

    0[||||||||69.3%]   4[||       2.0%]   7[          0.0%] 11[|||      15.3%]
    1[||||||||66.4%]   5[||       2.0%]   8[|         0.7%] 12[||        5.3%]
    2[||||||||73.3%]   6[||       2.0%]   9[|||      17.1%] 13[||        5.3%]
    3[||||||||70.7%]                     10[||||     20.0%]
  Mem[|||||||||||||||||||||19.8G/24.0G] Tasks: 690, 5177 thr, 0 kthr; 3 runnin
  Swp[|||||||||||||||||||||9.47G/11.0G] Load average: 7.95 7.01 6.61
                                        Uptime: 13 days, 10:13:57

  [Main]
  PID USER       PRI  NI  VIRT   RES S  CPU%▽MEM%   TIME+  Command
86489 jeroen      24   0  421G  236M R   2.0  1.0  3h09:59 /Applications/Adobe I
16870 jeroen      24   0 1577G  436M R   1.4  1.8  1h36:11 /Applications/Obsidia
10921 jeroen      17   0  416G  127M S   1.0  0.5  1h22:24 /Applications/Elgato
65086 jeroen      26   0  420G  455M S   0.3  1.9  0:26.27 claude --dangerously-
76651 jeroen      17   0  416G  142M S   0.1  0.6  2:49.96 /Applications/WhatsAp
16868 jeroen      17   0  447G 39568 S   0.1  0.2 11:00.40 /Applications/Obsidia
56514 jeroen      24   0  419G 55744 S   0.1  0.2  8:04.79 /Applications/DaVinci
34167 jeroen      17   0  421G  186M S   0.1  0.8 21:03.66 /Applications/zoom.us
27311 jeroen      17   0  415G 62912 S   0.1  0.2  4:47.99 /Applications/iTerm.a
27185 jeroen      17   0  396G 47792 S   0.0  0.2  0:46.33 /Applications/kitty.a
 1794 jeroen      17   0  419G 13328 S   0.0  0.1  9:30.57 osascript /Users/jero
34213 jeroen      24   0  415G 29888 S   0.0  0.1  4:16.67 /Applications/zoom.us
F1Help  F2Setup F3SearchF4FilterF5Tree  F6SortByF7Nice -F8Nice +F9Kill  F10Quit
```

## Cleanup
