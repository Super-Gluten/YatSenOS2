
file esp/KERNEL.ELF
gef config context.layout "-legend regs -stack code -args source -threads -trace extra memory"
gef-remote --qemu-user --qemu-binary esp/KERNEL.ELF localhost 1234
tmux-setup
b ysos_kernel::init
