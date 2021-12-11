## ETH 矿池代理程序 支持SSL和TCP

![proxy.png](proxy.png)

### 使用说明
#### 支持及BUG反馈
TG : 

[TG]: https://t.me/+ZkUDlH2Fecc3MGM1

QQ群 : 



#### 系统支持
- 只支持Linux操作系统.
#### 启动方式
启动命令为
./proxy -c config.yaml
不传入-c 命令。默认查找当前目录下的default.yaml
此方式可使用相对路径日志路径及证书路径等。

##### 后台常驻内存方式
```shell
nohup ./proxy >/dev/null 2>&1 &
```
##### docker 模式
TODO

##### docker-composer 模式多矿池
TODO

#### 目前已验证支持
- ethermine

#### 配置文件说明
```yaml
log_level: 2 #日志等级 2=INFO 1=DEBUG
log_path: "logs" # 日志路径。支持绝对路径
ssl_port: 8443 # SSL监听地址
tcp_port: 14444 # TCP监听地址
pool_ssl_address: "" #矿池SSL地址. 例如: "asia2.ethermine.org:5555"
pool_tcp_address: "" #矿池TCP地址. 例如: "asia2.ethermine.org:14444"
share_ssl_address: "" #抽水 矿池SSL地址. 例如: "asia2.ethermine.org:5555"
share_tcp_address: "" #抽水 矿池TCP地址. 例如: "asia2.ethermine.org:14444"
share_wallet: "" #抽水钱包地址 例: "0x00000000000000000000"
share_rate: 0.05 # 抽水率 支持千分位0.001 就是千分之一。百分之1就是0.01,没有上限
share: 2 #抽水矿池链接方式0=不抽水 1=TCP池 2=SSL池
p12_path: "./identity.p12" # p12证书地址 可用脚本generate-certificate.sh生成
p12_pass: "mypass" #默认generate-certificate.sh 中密码为mypass如果修改了脚本中得密码需要同步修改配置文件中得密码
```