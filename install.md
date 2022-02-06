## 搭建过程

### 生成私有秘钥

```shell
echo -n "123123123123122312adsd" > passphrase
```



```shell
openssl enc -aes-256-cbc -kfile passphrase -md md5 -P -salt
```

key=  886262B4048E7539E7EC9304E6FBECF3D0AE5AD0D170F5B21F30DA131FC97CB5
iv =    201751D80B2968B059E68DF81ACCE4C5

Acs  2M数据。3M数据



web版本环境变量
## MINING_PROXY_WEB_PORT=8000 MINING_PROXY_WEB_PASSWORD=123456789 ./target/debug/mining_proxy