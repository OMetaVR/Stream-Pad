#include "store.h"

#include "keytable.h"

#include <QFileInfo>
#include <QJSEngine>
#include <QJsonArray>
#include <QUrl>

#include <algorithm>

using namespace Qt::StringLiterals;

namespace {

OverlayStore *s_instance = nullptr;

// Image-provider id for an icon file (see IconProvider); empty when there is no icon.
QString iconId(const QJsonValue &path)
{
    const QString file = path.toString();
    return file.isEmpty() ? QString() : QString::fromLatin1(QUrl::toPercentEncoding(file));
}

QString colorName(const QJsonValue &value)
{
    const QColor color = parseColor(value, QColor());
    return color.isValid() ? color.name(QColor::HexArgb) : QString();
}

QVariantMap normalizeKey(const streampad::PadKey &pad, const QJsonObject &key)
{
    const QJsonObject legend = key.value(u"legend").toObject();
    const QString cover = key.value(u"cover").toString();
    const QString coverSuffix = QFileInfo(cover).suffix().toLower();
    const QJsonObject crop = key.value(u"coverCrop").toObject();
    const bool hasCrop = !crop.isEmpty() && crop.value(u"w").toDouble() > 0 && crop.value(u"h").toDouble() > 0;
    QString kind = key.value(u"kind").toString();
    if (kind != u"macro" && kind != u"global")
        kind = u"empty"_s;
    return {
        {u"label"_s, key.value(u"label").toString().trimmed()},
        {u"iconId"_s, iconId(key.value(u"icon"))},
        {u"iconTint"_s, key.value(u"iconTint").toBool()},
        {u"coverUrl"_s, cover.isEmpty() ? QString() : QUrl::fromLocalFile(cover).toString()},
        {u"coverAnimated"_s, coverSuffix == u"gif" || coverSuffix == u"webp"},
        {u"coverCrop"_s, hasCrop ? QVariant(crop.toVariantMap()) : QVariant()},
        {u"color"_s, colorName(key.value(u"color"))},
        {u"kind"_s, kind},
        {u"legendMain"_s, legend.contains(u"main") ? legend.value(u"main").toString() : QString::fromUtf8(pad.legendMain)},
        {u"legendSub"_s,
         legend.contains(u"sub") ? legend.value(u"sub").toString() : QString::fromUtf8(pad.legendSub ? pad.legendSub : "")},
    };
}

QVariantMap normalizeProfile(const QJsonObject &profile)
{
    const QJsonObject keys = profile.value(u"keys").toObject();
    QVariantMap normalizedKeys;
    for (const streampad::PadKey &pad : streampad::padKeys()) {
        const QString id = QString::fromLatin1(pad.id);
        normalizedKeys.insert(id, normalizeKey(pad, keys.value(id).toObject()));
    }
    return {
        {u"id"_s, profile.value(u"id").toString()},
        {u"name"_s, profile.value(u"name").toString().trimmed()},
        {u"iconId"_s, iconId(profile.value(u"icon"))},
        {u"keys"_s, normalizedKeys},
    };
}

} // namespace

OverlayStore::OverlayStore(QObject *parent) : QObject(parent)
{
    Q_ASSERT(!s_instance);
    s_instance = this;
    m_profile = normalizeProfile({});
}

OverlayStore::~OverlayStore()
{
    s_instance = nullptr;
}

OverlayStore *OverlayStore::create(QQmlEngine *, QJSEngine *engine)
{
    Q_ASSERT(s_instance && engine->thread() == s_instance->thread());
    QJSEngine::setObjectOwnership(s_instance, QJSEngine::CppOwnership);
    return s_instance;
}

QVariantList OverlayStore::keyLayout() const
{
    QVariantList layout;
    for (const streampad::PadKey &pad : streampad::padKeys()) {
        layout.append(QVariantMap{
            {u"id"_s, QString::fromLatin1(pad.id)},
            {u"col"_s, pad.col},
            {u"row"_s, pad.row},
            {u"colSpan"_s, pad.colSpan},
            {u"rowSpan"_s, pad.rowSpan},
        });
    }
    return layout;
}

void OverlayStore::setConnected(bool connected)
{
    if (connected == m_connected)
        return;
    const bool wasVisible = visible();
    m_connected = connected;
    m_pendingDirection = 0;
    emit connectedChanged();
    if (visible() != wasVisible)
        emit visibleChanged();
}

void OverlayStore::setStateVisible(bool stateVisible)
{
    const bool wasVisible = visible();
    m_stateVisible = stateVisible;
    if (visible() != wasVisible)
        emit visibleChanged();
}

void OverlayStore::applyState(const QJsonObject &state)
{
    const QJsonObject overlay = state.value(u"overlay").toObject();

    const OverlaySettings settings = OverlaySettings::fromJson(overlay.value(u"settings").toObject());
    if (settings != m_settings) {
        m_settings = settings;
        emit settingsChanged();
    }
    const QColor accent = parseColor(overlay.value(u"accent"), m_settings.accent);
    if (accent != m_accent) {
        m_accent = accent;
        emit accentChanged();
    }
    const QString output = overlay.value(u"output").toString();
    if (output != m_output) {
        m_output = output;
        emit outputChanged();
    }

    const int count = int(state.value(u"profiles").toArray().size());
    const int index = std::clamp(state.value(u"profileIndex").toInt(), 0, std::max(0, count - 1));
    const QVariantMap profile = normalizeProfile(state.value(u"profile").toObject());
    const bool switched = profile.value(u"id"_s) != m_profile.value(u"id"_s);
    if (switched)
        m_profileDirection = m_pendingDirection != 0 ? m_pendingDirection : (index >= m_profileIndex ? 1 : -1);
    m_pendingDirection = 0;

    if (index != m_profileIndex || count != m_profileCount) {
        m_profileIndex = index;
        m_profileCount = count;
        emit profilesChanged();
    }
    if (profile != m_profile) {
        m_profile = profile;
        emit profileChanged();
    }

    // Last, so geometry and content are current by the time the window is mapped.
    setStateVisible(overlay.value(u"visible").toBool());
}

void OverlayStore::applyEvent(const QString &event, const QJsonObject &data)
{
    if (event == u"key") {
        const streampad::PadKey *pad = streampad::findPadKey(data.value(u"key").toString());
        if (pad)
            emit keyEvent(QString::fromLatin1(pad->id), data.value(u"pressed").toBool(), data.value(u"kind").toString());
    } else if (event == u"profile") {
        m_pendingDirection = data.value(u"direction").toInt() < 0 ? -1 : 1;
    } else if (event == u"toast") {
        const QString text = data.value(u"text").toString().trimmed();
        if (text.isEmpty())
            return;
        QString kind = data.value(u"kind").toString();
        if (kind != u"success" && kind != u"error")
            kind = u"info"_s;
        emit toastReceived({
            {u"id"_s, data.value(u"id").toVariant()},
            {u"text"_s, text},
            {u"kind"_s, kind},
            {u"durationMs"_s, std::clamp(data.value(u"durationMs").toInt(1600), 600, 20000)},
            {u"iconId"_s, iconId(data.value(u"icon"))},
        });
    }
}
