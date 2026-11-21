# Безопасность

Об уязвимости сообщай не в открытом issue, а через [Security → Report a vulnerability](https://github.com/Redox-openDriver/gestus/security/advisories/new)
на GitHub: сообщение увидят только сопровождающие. Опиши, что нужно сделать, чтобы воспроизвести, и что из этого
следует.

Библиотека не запускает программ: действия жестов — только щелчки, клавиши и переключатели режимов. Ввода-вывода у
неё нет, кроме чтения файлов настроек в `config::load` (не больше 1 МиБ, каждое поле проверяется). Модель угроз
драйвера, который ею пользуется, — в
[elantp-327e/SECURITY.md](https://github.com/Redox-openDriver/elantp-327e/blob/main/SECURITY.md).
