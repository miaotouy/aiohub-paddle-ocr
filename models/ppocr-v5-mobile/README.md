# ppocr-v5-mobile 模型目录

首版发布 ZIP 需要在本目录放置以下文件：

```txt
det.mnn
rec.mnn
keys.txt
```

源码仓库不提交真实模型文件。打包脚本会在 `--package` 时校验这些文件存在且大小不为 0。
