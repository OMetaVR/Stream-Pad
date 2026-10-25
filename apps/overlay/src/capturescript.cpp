#include "capturescript.h"

#include "keytable.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

#include <algorithm>

namespace streampad {

namespace {

constexpr int kMaxFrames = 100000;

bool parseEvent(const QJsonObject &json, const QDir &base, int frames, CaptureEvent *event, QString *error)
{
    const QString type = json.value(u"type").toString();
    event->frame = json.value(u"frame").toInt(-1);
    if (event->frame < 0 || event->frame >= frames) {
        *error = QStringLiteral("event '%1' has frame %2 outside 0..%3").arg(type).arg(event->frame).arg(frames - 1);
        return false;
    }
    if (type == u"visible") {
        event->type = CaptureEvent::Type::Visible;
        event->flag = json.value(u"value").toBool(true);
    } else if (type == u"key") {
        event->type = CaptureEvent::Type::Key;
        event->text = json.value(u"key").toString();
        event->flag = json.value(u"pressed").toBool(true);
        event->kind = json.value(u"kind").toString(QStringLiteral("macro"));
        if (!findPadKey(event->text)) {
            *error = QStringLiteral("unknown key '%1'").arg(event->text);
            return false;
        }
    } else if (type == u"toast") {
        event->type = CaptureEvent::Type::Toast;
        event->text = json.value(u"text").toString();
        event->kind = json.value(u"kind").toString(QStringLiteral("info"));
        event->number = json.value(u"durationMs").toInt(1600);
    } else if (type == u"profile") {
        event->type = CaptureEvent::Type::Profile;
        event->text = base.absoluteFilePath(json.value(u"state").toString());
        event->number = json.value(u"direction").toInt(1) < 0 ? -1 : 1;
        if (!QFileInfo(event->text).isFile()) {
            *error = QStringLiteral("profile state '%1' not found").arg(event->text);
            return false;
        }
    } else {
        *error = QStringLiteral("unknown event type '%1' (expected visible, key, toast or profile)").arg(type);
        return false;
    }
    return true;
}

} // namespace

bool CaptureScript::load(const QString &path, QString *error)
{
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly)) {
        *error = QStringLiteral("cannot read %1: %2").arg(path, file.errorString());
        return false;
    }
    QJsonParseError parseError;
    const QJsonDocument doc = QJsonDocument::fromJson(file.readAll(), &parseError);
    if (!doc.isObject()) {
        *error = QStringLiteral("invalid JSON in %1: %2").arg(path, parseError.errorString());
        return false;
    }
    const QJsonObject json = doc.object();
    const QDir base = QFileInfo(path).absoluteDir();

    fps = json.value(u"fps").toInt(30);
    frames = json.value(u"frames").toInt(0);
    if (fps < 1 || fps > 240 || frames < 1 || frames > kMaxFrames) {
        *error = QStringLiteral("'fps' must be 1..240 and 'frames' 1..%1").arg(kMaxFrames);
        return false;
    }
    statePath = base.absoluteFilePath(json.value(u"state").toString());
    if (!QFileInfo(statePath).isFile()) {
        *error = QStringLiteral("initial state '%1' not found").arg(statePath);
        return false;
    }
    events.clear();
    for (const QJsonValue &value : json.value(u"events").toArray()) {
        CaptureEvent event;
        if (!parseEvent(value.toObject(), base, frames, &event, error))
            return false;
        events.append(event);
    }
    std::stable_sort(events.begin(), events.end(),
                     [](const CaptureEvent &a, const CaptureEvent &b) { return a.frame < b.frame; });
    return true;
}

} // namespace streampad
