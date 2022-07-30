### 实验总结

实现了三个系统调用，sys_linkat，sys_unlinkat，sys_stat。

sys_linkat的思路就是在根目录上新建一个目录项，目录项的名字是硬链接的名字，inode_number就是原链接的inode_number。

sys_unlinkat和sys_stat涉及到统计文件链接数目的一个功能。实现的细节就是遍历根目录的目录项，检查其inode_number是否与给定文件的inode_number相同，如果相同就加一。

sys_unlinkat中删除目录项采用了最简单的实现方式，直接在要删除的目录项上复写一个空的目录项。没有实现目录项的搬运，也没有更新目录的大小。

sys_stat最关键的是通过文件描述符获得inode_number。实现的细节是给file trait加一个方法get_inode_number，让OSInode实现即可(ps：文件描述符表存的对象就是OSInode类型的)。

### 问答作业

1. root的inode起着索引全局的作用的，所有的目录，文件都要从root开始索引，如果root inode中的内容损坏了，整个目录就无法索引了。

下面是文档第七章的问答作业

1. linux中的管道命令|，例如```cat file.txt | grep hello"
2. 可以使用消息队列。

